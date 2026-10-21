<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { app } from '../state.svelte';
  import type { EmulatorInfo, LauncherConfig, LauncherInfo, Runner } from '../api/contracts';
  import type { PortEntry } from '../types';
  import { launcherDraft, launcherMatches, launcherTemplates } from '../launchers';
  import { coverSrc } from '../api/map';
  import { platformLabel, systemLabel } from '../labels';
  import BiosPanel from '../components/BiosPanel.svelte';
  import EmulatorDialog from '../components/EmulatorDialog.svelte';
  import InstallMenu from '../components/InstallMenu.svelte';
  import LutrisSource from '../components/LutrisSource.svelte';
  import PurpleLauncher from '../components/PurpleLauncher.svelte';
  import ToolCard from '../components/ToolCard.svelte';
  import Icon from '../components/Icon.svelte';

  // Everything that opens, runs or hosts games is one kind of thing here; what separates
  // them is the platform they serve, exactly like the Library.
  type Item =
    | { t: 'emulator'; key: string; name: string; platforms: string[]; ready: boolean; e: EmulatorInfo }
    | { t: 'runner'; key: string; name: string; platforms: string[]; ready: boolean; r: Runner }
    | { t: 'launcher'; key: string; name: string; platforms: string[]; ready: boolean; l: LauncherInfo }
    | { t: 'port'; key: string; name: string; platforms: string[]; ready: boolean; p: PortEntry }
    | { t: 'source'; key: string; name: string; platforms: string[]; ready: boolean; src: 'purple' | 'lutris' };

  const norm = (id: string) => (id.startsWith('pc-') ? 'pc' : id);
  const labelOf = (id: string) => (id === 'pc' ? 'PC' : ((platformLabel as Record<string, string>)[id] ?? systemLabel(id)));
  const runnerSystem = (r: Runner) => (r.kind === 'duckstation' ? 'ps1' : 'ps2');

  let query = $state('');
  let onlyReady = $state(false);
  let platform = $state('');
  let sourceDialog = $state<'purple' | 'lutris' | null>(null);
  let draft = $state<LauncherConfig | null>(null);
  let error = $state<string | null>(null);
  let saving = $state(false);
  let nameInput = $state<HTMLInputElement>();

  const lutrisOn = $derived(!!app.lutrisSource?.config.enabled);
  const all = $derived.by<Item[]>(() => [
    ...app.emulators.map((e): Item => ({ t: 'emulator', key: `emu:${e.id}`, name: e.name, platforms: e.systems, ready: e.status !== 'not_installed', e })),
    ...app.runners.filter((r) => r.source !== 'marquee').map((r): Item => ({ t: 'runner', key: `run:${r.id}`, name: r.display_name, platforms: [runnerSystem(r)], ready: r.available, r })),
    ...app.launchers.map((l): Item => ({ t: 'launcher', key: `app:${l.id}`, name: l.name, platforms: ['pc'], ready: l.available, l })),
    ...app.ports.map((p): Item => ({ t: 'port', key: `port:${p.id}`, name: p.name, platforms: [norm(p.platform)], ready: p.status === 'ready' || p.status === 'update', p })),
    { t: 'source', key: 'src:purple', name: 'Purple for Aion 2', platforms: ['pc'], ready: false, src: 'purple' },
    { t: 'source', key: 'src:lutris', name: 'Lutris library', platforms: ['pc'], ready: lutrisOn, src: 'lutris' },
  ]);
  const matchesText = (i: Item) => {
    const q = query.trim().toLowerCase();
    if (!q) return true;
    const extra = i.t === 'port' ? i.p.game : i.t === 'launcher' ? i.l.executable : '';
    return `${i.name} ${extra} ${i.platforms.map(labelOf).join(' ')}`.toLowerCase().includes(q);
  };
  const items = $derived(
    all
      .filter((i) => matchesText(i) && (!onlyReady || i.ready) && (!platform || i.platforms.includes(platform)))
      .sort((a, b) => Number(b.ready) - Number(a.ready) || a.name.localeCompare(b.name)),
  );
  const platforms = $derived(
    [...new Set(all.flatMap((i) => i.platforms))].sort((a, b) => labelOf(a).localeCompare(labelOf(b))),
  );
  const ready = $derived(all.filter((i) => i.ready).length);
  onMount(() => { void app.loadLaunchers(); void app.loadEmulators(); });
  $effect(() => {
    if (!draft && !sourceDialog) return;
    app.closeLauncherEditor = () => { if (sourceDialog) sourceDialog = null; else if (!saving) draft = null; };
    return () => { app.closeLauncherEditor = null; };
  });

  async function edit(info?: LauncherConfig) {
    error = null; draft = launcherDraft(info);
    await tick(); nameInput?.focus(); nameInput?.scrollIntoView({ block: 'center' });
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault(); if (!draft || saving) return;
    saving = true; error = null;
    try { await app.saveLauncher({ ...draft, working_directory: draft.working_directory?.trim() || null }); draft = null; }
    catch (err) { error = err instanceof Error ? err.message : 'Could not save launcher'; }
    finally { saving = false; }
  }
  async function browse() {
    const id = draft?.id;
    try { const path = await app.api.pickExecutable(); if (path && draft && draft.id === id) draft.executable = path; }
    catch { error = 'Could not open the file picker. Enter the executable path instead.'; }
  }
  async function pin(info: LauncherInfo) {
    const { id, name, executable, args, working_directory, pinned } = info;
    try { await app.saveLauncher({ id, name, executable, args, working_directory, pinned: !pinned }); }
    catch (err) { app.notify(err instanceof Error ? err.message : 'Could not pin launcher'); }
  }
</script>

<div class="tools">
  <header class="tool-hero">
    <div class="hero-copy"><h1>Tools</h1><p>Emulators, launchers, native ports and connected libraries, by the platform they run. {ready} ready.</p></div>
    <InstallMenu />
    <button class="btn primary" data-nav onclick={() => edit()}><Icon name="plus" /> Add launcher</button>
  </header>

  {#if draft}
    <div class="modal-wrap" role="dialog" aria-modal="true" aria-label="Launcher configuration" tabindex="-1" onkeydown={(e) => { if (e.key === "Escape" && !saving) draft = null; }} onclick={(e) => { if (e.target === e.currentTarget && !saving) draft = null; }}>
    <form class="editor glass-panel" onsubmit={submit} aria-label="Launcher configuration" aria-busy={saving}>
      <div class="section-heading"><div><span class="eyebrow">Configuration</span><h2>{app.launchers.some((l) => l.id === draft?.id) ? 'Edit launcher' : 'Add a launcher'}</h2></div><button class="btn glass small" type="button" data-nav disabled={saving} onclick={() => { draft = null; }} aria-label="Close launcher editor"><Icon name="x" /></button></div>
      <p>Open an existing application, AppImage, or native game. Saving does not start it.</p>
      <div class="templates" data-nav-row><span>Start with</span>{#each launcherTemplates as template}<button class="chip" type="button" data-nav disabled={saving} onclick={() => { if (draft) draft = { ...draft, ...template, args: [...template.args] }; }}>{template.name}</button>{/each}</div>
      <div class="fields">
        <label>Name<input bind:this={nameInput} bind:value={draft.name} data-nav required maxlength={100} placeholder="My launcher" disabled={saving} /></label>
        <label class="wide">Executable<div class="path-field"><input bind:value={draft.executable} data-nav required placeholder="Command name or /absolute/path/to/AppImage" disabled={saving} />{#if !app.demo}<button class="btn glass small" type="button" data-nav onclick={browse} disabled={saving}>Browse</button>{/if}</div></label>
        <label class="wide">Working directory <span class="optional">optional</span><input value={draft.working_directory ?? ''} oninput={(e) => { if (draft) draft.working_directory = e.currentTarget.value; }} data-nav placeholder="Application folder, if required" disabled={saving} /></label>
      </div>
      <fieldset disabled={saving}><legend>Arguments <span class="optional">optional · one value per field</span></legend>
        {#each draft.args as arg, i (i)}<div class="argument"><input value={arg} oninput={(e) => { if (draft) draft.args[i] = e.currentTarget.value; }} data-nav aria-label={`Argument ${i + 1}`} /><button class="btn glass small" type="button" data-nav onclick={() => { if (draft) draft.args = draft.args.filter((_, n) => n !== i); }} aria-label={`Remove argument ${i + 1}`}><Icon name="x" size={16} /></button></div>{/each}
        <button class="btn glass small" type="button" data-nav disabled={draft.args.length >= 64} onclick={() => { if (draft) draft.args = [...draft.args, '']; }}><Icon name="plus" size={16} /> Add argument</button>
      </fieldset>
      <label class="pin-choice"><input type="checkbox" bind:checked={draft.pinned} data-nav disabled={saving} /> Pin near the top</label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <footer><span>For Flatpak, choose the flatpak executable and add its run arguments. Windows apps need a configured compatibility launcher.</span><button class="btn glass" type="button" data-nav disabled={saving} onclick={() => { draft = null; }}>Cancel</button><button class="btn primary" type="submit" data-nav disabled={saving}>{saving ? 'Saving…' : 'Save launcher'}</button></footer>
    </form>
    </div>
  {/if}

  <div class="toolbar glass-panel">
    <label class="search"><Icon name="search" size={18} /><input bind:value={query} data-nav aria-label="Search tools" placeholder="Search emulators, launchers, ports…" /></label>
    <button class="chip" class:on={onlyReady} data-nav aria-pressed={onlyReady} onclick={() => (onlyReady = !onlyReady)}>Ready only</button>
    <button class="btn glass small" data-nav disabled={app.launchersLoading} onclick={() => { void app.loadLaunchers(); void app.loadEmulators(); }}><Icon name="refresh" size={16} /> Refresh</button>
  </div>
  <div class="filters" data-nav-row aria-label="Platforms">
    <button class="chip" class:on={!platform} data-nav aria-pressed={!platform} onclick={() => (platform = '')}>All platforms <span>{all.length}</span></button>
    {#each platforms as p}<button class="chip" class:on={platform === p} data-nav aria-pressed={platform === p} onclick={() => (platform = p)}>{labelOf(p)} <span>{all.filter((i) => i.platforms.includes(p)).length}</span></button>{/each}
  </div>

  {#if app.launcherError}<div class="empty glass-panel" role="alert"><p>{app.launcherError}</p><button class="btn glass" data-nav onclick={() => app.loadLaunchers()}>Try again</button></div>{/if}
  <div class="tool-grid">
    {#each items as item (item.key)}
      {#if item.t === 'emulator'}
        {@const e = item.e}
        {@const busy = app.emulatorBusy[e.id]}
        <ToolCard seed={`emulator:${e.id}`} kind="Emulator" title={e.name} subtitle={e.systems.map(labelOf).join(' · ')}
          detail={`${e.integrated ? 'Launches your library games' : 'Opens on its own for now'}${e.installed_version ? ` · ${e.installed_version.length > 18 ? `${e.installed_version.slice(0, 14)}…` : e.installed_version}` : ''}`}
          badge={{ text: e.status === 'not_installed' ? 'Not installed' : e.status === 'update' ? 'Update available' : 'Ready', tone: e.status === 'ready' ? 'ok' : e.status === 'update' ? 'warn' : '' }}
          icon={coverSrc(e.icon)} ready={item.ready} {busy} onopen={() => (app.emulatorDialogId = e.id)}>
          {#snippet actions()}
            {#if e.status === 'not_installed'}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}><Icon name="download" size={15} />{busy ? 'Installing…' : 'Install'}</button>
            {:else if e.status === 'update'}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.installEmulator(e)}>Update</button>
            {:else}
              <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.openEmulator(e)}><Icon name="play" size={15} />Open</button>
            {/if}
            <button class="btn glass small" data-nav onclick={() => (app.emulatorDialogId = e.id)}><Icon name="settings" size={15} />Configure</button>
          {/snippet}
        </ToolCard>
      {:else if item.t === 'runner'}
        {@const r = item.r}
        {@const isDefault = app.runnerFor(runnerSystem(r))?.id === r.id}
        <ToolCard seed={`runner:${r.id}`} kind="Emulator" title={r.display_name} subtitle={labelOf(runnerSystem(r))} detail={r.bios_state.replaceAll('_', ' ')} mono={r.command.join(' ')}
          badge={{ text: isDefault ? 'Default' : r.available ? 'Detected' : 'Unavailable', tone: r.available ? 'ok' : '' }} ready={r.available}>
          {#snippet actions()}
            <button class="btn primary small" data-nav disabled={!r.available || isDefault} onclick={() => app.chooseRunner(r.id)}>Use as default</button>
            <button class="btn glass small" data-nav disabled={!r.available} onclick={() => app.openSetup(r.id)}>Setup</button>
          {/snippet}
        </ToolCard>
      {:else if item.t === 'launcher'}
        {@const l = item.l}
        <ToolCard seed={`launcher:${l.id}`} kind="Launcher" title={l.name} subtitle={l.status} mono={l.executable} detail={l.args.length ? `${l.args.length} argument${l.args.length === 1 ? '' : 's'} configured` : ''}
          badge={{ text: l.available ? 'Ready' : app.demo ? 'Demo' : 'Needs setup', tone: l.available ? 'ok' : '' }} glyph="monitor" ready={l.available}>
          {#snippet actions()}
            <button class="btn primary small" data-nav disabled={!l.available || app.openingLaunchers.includes(l.id)} onclick={() => app.openLauncher(l.id)}><Icon name="play" size={15} />{app.openingLaunchers.includes(l.id) ? 'Opening…' : 'Open'}</button>
            <button class="btn glass small" data-nav onclick={() => edit(l)}>Configure</button>
            {#if l.configured}<button class="text-button" data-nav onclick={() => app.removeLauncher(l.id)}>{l.id.startsWith('detected.') ? 'Reset' : 'Remove'}</button>{/if}
            <button class="pin" class:pinned={l.pinned} data-nav aria-label={`${l.pinned ? 'Unpin' : 'Pin'} ${l.name}`} aria-pressed={l.pinned} onclick={() => pin(l)}><Icon name="star" size={18} filled={l.pinned} /></button>
          {/snippet}
        </ToolCard>
      {:else if item.t === 'port'}
        {@const p = item.p}
        <ToolCard seed={`port:${p.id}`} kind="Native port" title={p.name} subtitle={p.game} detail={p.requirement}
          badge={{ text: p.status.replaceAll('-', ' '), tone: p.status === 'ready' ? 'ok' : p.status === 'update' ? 'warn' : '' }} glyph="puzzle" ready={item.ready || p.status !== 'not-installed'}>
          {#snippet actions()}
            <button class="btn glass small" data-nav onclick={() => app.go('ports')}>Manage</button>
            {#if !app.demo && p.status !== 'not-installed'}<button class="btn glass small" data-nav onclick={() => app.openPortFolder(p)}>Folder</button>{/if}
          {/snippet}
        </ToolCard>
      {:else}
        {@const purple = item.src === 'purple'}
        <ToolCard seed={`source:${item.src}`} kind="Source" title={item.name}
          subtitle={purple ? 'Managed Windows launcher' : 'Existing installations and synced store libraries'}
          detail={purple ? 'Sets up the compatibility runtime and a separate Purple install.' : 'Discovered automatically; your Lutris settings are kept.'}
          badge={{ text: purple ? 'Windows' : lutrisOn ? 'Connected' : 'Not connected', tone: !purple && lutrisOn ? 'ok' : '' }} glyph={purple ? 'monitor' : 'grid'} ready={item.ready || purple}>
          {#snippet actions()}
            <button class="btn glass small" data-nav onclick={() => (sourceDialog = item.src)}>{purple ? 'Set up / open' : 'Configure'}</button>
          {/snippet}
        </ToolCard>
      {/if}
    {:else}
      <div class="empty glass-panel"><Icon name="controller" size={28} /><h3>{app.launchersLoading ? 'Finding your tools…' : 'Nothing matches'}</h3><p>Change the platform or search, or add a launcher.</p><button class="btn glass" data-nav onclick={() => edit()}>Add launcher</button></div>
    {/each}
  </div>

  <BiosPanel {platform} />
  <EmulatorDialog />

  {#if sourceDialog}
    <div class="modal-wrap" role="dialog" aria-modal="true" aria-label="Source setup" tabindex="-1" onkeydown={(e) => { if (e.key === 'Escape') sourceDialog = null; }} onclick={(e) => { if (e.target === e.currentTarget) sourceDialog = null; }}>
      <div class="editor source-dialog">
        <button class="btn glass small close" data-nav onclick={() => (sourceDialog = null)} aria-label="Close"><Icon name="x" /></button>
        {#if sourceDialog === 'purple'}<PurpleLauncher />{:else}<LutrisSource />{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .tools { padding:32px var(--gutter) 64px; display:grid; gap:30px; max-width:1600px; margin:auto; }
  .tool-hero { display:flex; flex-wrap:wrap; align-items:center; gap:22px; }
  .hero-copy { flex:1; min-width:240px; } h1 { margin:5px 0 8px; font-size:clamp(28px, 3vw, 40px); letter-spacing:-1px; } h2 { margin:0; font-size:21px; letter-spacing:-.4px; } h3 { margin:0 0 8px; font-size:17px; }
  p { color:var(--text-soft); margin:5px 0; line-height:1.6; }
  .filters { display:flex; flex-wrap:wrap; gap:8px; margin-top:-14px; } .filters .chip span { opacity:.6; margin-left:4px; } .source-dialog { position:relative; } .source-dialog .close { position:absolute; top:14px; right:14px; z-index:1; }
  .toolbar { display:flex; flex-wrap:wrap; gap:12px; padding:12px; border-radius:16px; align-items:center; } .search { display:flex; align-items:center; gap:10px; flex:1; min-width:220px; color:var(--text-dim); padding:0 8px; } .search input { background:transparent; border:0; padding:10px 0; }
  input { min-width:0; width:100%; border:1px solid var(--line); background:var(--field); color:var(--text); border-radius:10px; padding:12px; font:inherit; box-sizing:border-box; } input:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
  .section-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; margin-bottom:18px; } .tool-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(275px,1fr)); gap:16px; }
  
  
  .pin, .text-button { border:0; background:none; color:var(--text-dim); padding:8px; cursor:pointer; } .pin.pinned { color:var(--warn); } .text-button { font:inherit; font-size:12px; }
  .empty { grid-column:1/-1; text-align:center; padding:36px; border-radius:18px; color:var(--text-dim); display:grid; gap:12px; justify-items:center; } .empty h3, .empty p { margin:0; }
  .modal-wrap { position:fixed; inset:0; z-index:40; display:grid; place-items:start center; overflow:auto; padding:6vh 24px; background:rgb(0 0 0 / .6); backdrop-filter:var(--overlay-filter); } .modal-wrap .editor { width:min(820px,100%); } .editor { border-radius:20px; padding:26px; display:grid; gap:18px; } .editor .section-heading { margin:0; } .fields { display:grid; grid-template-columns:1fr 1fr; gap:18px; } .fields label { display:grid; gap:9px; font-size:13px; } .fields .wide { grid-column:1/-1; } .path-field { display:flex; gap:8px; } .path-field input { flex:1; } .optional { color:var(--text-dim); font-weight:normal; font-size:12px; } .templates { display:flex; align-items:center; flex-wrap:wrap; gap:8px; font-size:12px; color:var(--text-dim); }
  fieldset { border:1px solid var(--line); padding:16px; border-radius:12px; display:grid; gap:10px; } legend { padding:0 8px; font-size:13px; } .argument { display:flex; gap:8px; } .pin-choice { display:flex; gap:10px; align-items:center; font-size:13px; } input[type=checkbox] { width:18px; height:18px; accent-color:var(--accent); }
  footer { display:flex; align-items:center; flex-wrap:wrap; gap:10px; padding-top:16px; border-top:1px solid var(--line); } footer span { flex:1; min-width:220px; font-size:12px; color:var(--text-dim); line-height:1.6; } .error { color:var(--bad); } 
  @media(max-width:800px) { .tools { padding:22px var(--gutter); } .fields { grid-template-columns:1fr; } }
</style>
