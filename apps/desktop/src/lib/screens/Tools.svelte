<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { app } from '../state.svelte';
  import type { LauncherConfig, LauncherInfo } from '../api/contracts';
  import { launcherDraft, launcherMatches, launcherTemplates } from '../launchers';
  import InstallMenu from '../components/InstallMenu.svelte';
  import LutrisSource from '../components/LutrisSource.svelte';
  import PurpleLauncher from '../components/PurpleLauncher.svelte';
  import EmulatorsPanel from '../components/EmulatorsPanel.svelte';
  import Icon from '../components/Icon.svelte';

  const sections = ['launchers', 'emulators', 'ports'];
  let query = $state('');
  let onlyReady = $state(false);
  let draft = $state<LauncherConfig | null>(null);
  let error = $state<string | null>(null);
  let saving = $state(false);
  let nameInput = $state<HTMLInputElement>();
  const shown = $derived(app.launchers.filter((l) => launcherMatches(l, query, onlyReady)));
  const ports = $derived(app.ports.filter((p) => (!onlyReady || p.status === 'ready' || p.status === 'update') && `${p.name} ${p.game}`.toLowerCase().includes(query.toLowerCase())));
  const runners = $derived(app.runners.filter((r) => (!onlyReady || r.available) && r.display_name.toLowerCase().includes(query.toLowerCase())));
  const ready = $derived(app.launchers.filter((l) => l.available).length);
  onMount(() => { void app.loadLaunchers(); });
  $effect(() => {
    if (!draft) return;
    app.closeLauncherEditor = () => { if (!saving) draft = null; };
    return () => { app.closeLauncherEditor = null; };
  });

  function toggle(id: string) {
    app.toolSections = app.toolSections.includes(id) ? app.toolSections.filter((x) => x !== id) : [...app.toolSections, id];
  }
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
  <header class="tool-hero glass-panel">
    <div class="hero-icon"><Icon name="controller" size={34} /></div>
    <div class="hero-copy"><span class="eyebrow">Your play setup</span><h1>Tools & launchers</h1><p>Every way into your games, in one place.</p></div>
    <button class="btn primary" data-nav onclick={() => edit()}><Icon name="plus" /> Add launcher</button>
    <div class="summary"><span><i class="dot"></i>{ready} launcher{ready === 1 ? '' : 's'} ready</span><span>{app.runners.filter((r) => r.available).length} emulator runner{app.runners.filter((r) => r.available).length === 1 ? '' : 's'}</span><span>{app.ports.filter((p) => p.status !== 'not-installed').length} native ports installed</span></div>
  </header>

  {#if draft}
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
  {/if}

  <PurpleLauncher />
  <LutrisSource />

  <div class="toolbar glass-panel">
    <label class="search"><Icon name="search" size={18} /><input bind:value={query} data-nav aria-label="Search tools" placeholder="Search your tools…" /></label>
    <button class="chip" class:on={onlyReady} data-nav aria-pressed={onlyReady} onclick={() => (onlyReady = !onlyReady)}>Ready only</button>
    <button class="btn glass small" data-nav disabled={app.launchersLoading} onclick={() => app.loadLaunchers()}><Icon name="refresh" size={16} /> Refresh launchers</button>
    <div class="sections" data-nav-row aria-label="Visible tool sections">{#each sections as section}<button class="chip" class:on={app.toolSections.includes(section)} aria-pressed={app.toolSections.includes(section)} data-nav onclick={() => toggle(section)}>{section === 'ports' ? 'Native ports' : section[0].toUpperCase() + section.slice(1)}</button>{/each}</div>
  </div>

  {#if app.toolSections.includes('launchers')}
    <section aria-labelledby="launchers-title"><div class="section-heading"><div><h2 id="launchers-title">Your launchers</h2><p>Open apps here. Their game libraries remain separate until an import adapter is connected.</p></div><span class="pill">{shown.length}</span></div>
      {#if app.launcherError}<div class="empty glass-panel" role="alert"><p>{app.launcherError}</p><button class="btn glass" data-nav onclick={() => app.loadLaunchers()}>Try again</button></div>{/if}
      <div class="tool-grid">
        {#each shown as launcher (launcher.id)}
          <article class="tool-card glass-panel" class:ready={launcher.available}>
            <div class="card-heading"><span class="app-mark"><Icon name="monitor" size={24} /></span><div><h3>{launcher.name}</h3><span class="badge" class:available={launcher.available}>{launcher.available ? 'Ready' : app.demo ? 'Demo' : 'Needs setup'}</span></div><button class="pin" class:pinned={launcher.pinned} data-nav aria-label={`${launcher.pinned ? 'Unpin' : 'Pin'} ${launcher.name}`} aria-pressed={launcher.pinned} onclick={() => pin(launcher)}><Icon name="star" size={18} filled={launcher.pinned} /></button></div>
            <p class="status">{launcher.status}</p><p class="command" title={launcher.executable}>{launcher.executable}</p>
            {#if launcher.args.length}<span class="detail">{launcher.args.length} argument{launcher.args.length === 1 ? '' : 's'} configured</span>{/if}
            <div class="card-actions"><button class="btn primary small" data-nav disabled={!launcher.available || app.openingLaunchers.includes(launcher.id)} onclick={() => app.openLauncher(launcher.id)}><Icon name="play" size={15} />{app.openingLaunchers.includes(launcher.id) ? 'Opening…' : 'Open'}</button><button class="btn glass small" data-nav onclick={() => edit(launcher)}>Configure</button>{#if launcher.configured}<button class="text-button" data-nav onclick={() => app.removeLauncher(launcher.id)}>{launcher.id.startsWith('detected.') ? 'Reset' : 'Remove'}</button>{/if}</div>
          </article>
        {:else}
          <div class="empty glass-panel"><Icon name="monitor" size={28} /><h3>{app.launchersLoading ? 'Finding your launchers…' : 'No matching launchers'}</h3><p>Add an application or change the filters to see more.</p><button class="btn glass" data-nav onclick={() => edit()}>Add launcher</button></div>
        {/each}
      </div>
    </section>
  {/if}
  {#if app.toolSections.includes('emulators')}
    <EmulatorsPanel {query} {onlyReady} />
    <section aria-labelledby="emulators-title"><div class="section-heading"><div><h2 id="emulators-title">Emulators</h2><p>PS1 uses DuckStation; PS2 uses PCSX2. Defaults are saved per system.</p></div><InstallMenu /></div>
      <div class="tool-grid">{#each runners as runner}<article class="tool-card glass-panel"><div class="card-heading"><span class="app-mark"><Icon name="controller" size={24} /></span><div><h3>{runner.display_name}</h3><span class="badge" class:available={runner.available}>{app.runnerFor(runner.kind === 'duckstation' ? 'ps1' : 'ps2')?.id === runner.id ? 'Default' : runner.available ? 'Detected' : 'Unavailable'}</span></div></div><p class="status">{runner.bios_state.replaceAll('_', ' ')}</p><p class="command">{runner.command.join(' ')}</p><div class="card-actions"><button class="btn primary small" data-nav disabled={!runner.available || app.runnerFor(runner.kind === 'duckstation' ? 'ps1' : 'ps2')?.id === runner.id} onclick={() => app.chooseRunner(runner.id)}>Use as default</button><button class="btn glass small" data-nav disabled={!runner.available} onclick={() => app.openSetup(runner.id)}>Setup</button></div></article>{:else}<div class="empty glass-panel"><Icon name="controller" size={28} /><h3>{query || onlyReady ? 'No matching runners' : 'Set up an emulator'}</h3><p>Install DuckStation for PS1 or PCSX2 for PS2, then open its setup and add a disc folder in Settings.</p><button class="btn glass" data-nav onclick={() => app.go('settings')}>Open settings</button></div>{/each}</div>
    </section>
  {/if}
  {#if app.toolSections.includes('ports')}
    <section aria-labelledby="ports-title"><div class="section-heading"><div><h2 id="ports-title">Native ports</h2><p>Modern runtimes for your classic games.</p></div><button class="btn glass small" data-nav onclick={() => app.go('ports')}>Browse ports</button></div>
      <div class="tool-grid">{#each ports as port}<article class="tool-card glass-panel"><div class="card-heading"><span class="app-mark"><Icon name="puzzle" size={24} /></span><div><h3>{port.name}</h3><span class="badge" class:available={port.status === 'ready'}>{port.status.replaceAll('-', ' ')}</span></div></div><p class="status">{port.game}</p><p class="detail">{port.requirement}</p><div class="card-actions"><button class="btn glass small" data-nav onclick={() => app.go('ports')}>Manage</button>{#if !app.demo && port.status !== 'not-installed'}<button class="btn glass small" data-nav onclick={() => app.openPortFolder(port)}>Folder</button>{/if}</div></article>{:else}<div class="empty glass-panel"><p>No native ports match these filters.</p></div>{/each}</div>
      {#if !query && !onlyReady}<div class="opengoal glass-panel"><img src="/reference/jak/opengoal-logo.png" alt="" /><div><h3>Already have OpenGOAL?</h3><p>Add its installed launcher as an app shortcut. Managed game extraction remains separate.</p></div><button class="btn glass small" data-nav onclick={async () => { await edit(); if (draft) { draft.name = 'OpenGOAL'; } }}>Configure OpenGOAL</button></div>{/if}
    </section>
  {/if}
  {#if !app.toolSections.length}<div class="empty glass-panel"><h2>Your workspace, your choice</h2><p>Enable a section above to show your tools.</p></div>{/if}
</div>

<style>
  .tools { padding:32px var(--gutter) 64px; display:grid; gap:30px; max-width:1600px; margin:auto; }
  .tool-hero { display:flex; flex-wrap:wrap; align-items:center; gap:22px; padding:30px; border-radius:24px; background:radial-gradient(ellipse at 8% 0%, rgb(139 92 246 / .18), transparent 70%), var(--surface); }
  .hero-icon { width:72px; height:72px; border:1px solid rgb(139 92 246 / .35); color:var(--accent); background:rgb(139 92 246 / .1); border-radius:22px; display:grid; place-items:center; }
  .hero-copy { flex:1; min-width:240px; } h1 { margin:5px 0 8px; font-size:clamp(28px, 3vw, 40px); letter-spacing:-1px; } h2 { margin:0; font-size:21px; letter-spacing:-.4px; } h3 { margin:0 0 8px; font-size:17px; }
  p { color:var(--text-soft); margin:5px 0; line-height:1.6; } .summary { width:100%; display:flex; flex-wrap:wrap; gap:24px; padding-top:20px; border-top:1px solid var(--line); color:var(--text-dim); font-size:13px; } .summary span { display:flex; align-items:center; gap:8px; } .dot { width:7px; height:7px; border-radius:50%; background:var(--ok); }
  .toolbar { display:flex; flex-wrap:wrap; gap:12px; padding:12px; border-radius:16px; align-items:center; } .search { display:flex; align-items:center; gap:10px; flex:1; min-width:220px; color:var(--text-dim); padding:0 8px; } .search input { background:transparent; border:0; padding:10px 0; } .sections { display:flex; flex-wrap:wrap; gap:6px; padding-left:12px; border-left:1px solid var(--line); }
  input { min-width:0; width:100%; border:1px solid var(--line); background:var(--field); color:var(--text); border-radius:10px; padding:12px; font:inherit; box-sizing:border-box; } input:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
  .section-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; margin-bottom:18px; } .section-heading p { font-size:13px; } .tool-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(275px,1fr)); gap:16px; }
  .tool-card { padding:22px; border-radius:18px; display:flex; flex-direction:column; min-width:0; transition:border-color .2s; } .tool-card:hover { border-color:rgb(139 92 246 / .45); } .card-heading { display:flex; gap:14px; align-items:center; margin-bottom:16px; } .card-heading > div { flex:1; min-width:0; } .app-mark { display:grid; place-items:center; width:48px; height:48px; flex-shrink:0; border-radius:14px; background:var(--field); color:var(--text-soft); }
  .badge { display:inline-block; padding:4px 8px; border-radius:6px; font-size:11px; color:var(--text-dim); background:var(--field); } .badge.available { color:var(--ok); background:rgb(52 211 153 / .08); } .status { font-size:13px; } .command { font:12px ui-monospace,monospace; color:var(--text-dim); overflow-wrap:anywhere; margin:10px 0 16px; } .detail { font-size:12px; color:var(--text-dim); }
  .card-actions { display:flex; flex-wrap:wrap; gap:8px; margin-top:auto; padding-top:18px; } .pin, .text-button { border:0; background:none; color:var(--text-dim); padding:8px; cursor:pointer; } .pin.pinned { color:var(--warn); } .text-button { font:inherit; font-size:12px; }
  .empty { grid-column:1/-1; text-align:center; padding:36px; border-radius:18px; color:var(--text-dim); display:grid; gap:12px; justify-items:center; } .empty h3, .empty p { margin:0; }
  .editor { border-radius:20px; padding:26px; display:grid; gap:18px; } .editor .section-heading { margin:0; } .fields { display:grid; grid-template-columns:1fr 1fr; gap:18px; } .fields label { display:grid; gap:9px; font-size:13px; } .fields .wide { grid-column:1/-1; } .path-field { display:flex; gap:8px; } .path-field input { flex:1; } .optional { color:var(--text-dim); font-weight:normal; font-size:12px; } .templates { display:flex; align-items:center; flex-wrap:wrap; gap:8px; font-size:12px; color:var(--text-dim); }
  fieldset { border:1px solid var(--line); padding:16px; border-radius:12px; display:grid; gap:10px; } legend { padding:0 8px; font-size:13px; } .argument { display:flex; gap:8px; } .pin-choice { display:flex; gap:10px; align-items:center; font-size:13px; } input[type=checkbox] { width:18px; height:18px; accent-color:var(--accent); }
  footer { display:flex; align-items:center; flex-wrap:wrap; gap:10px; padding-top:16px; border-top:1px solid var(--line); } footer span { flex:1; min-width:220px; font-size:12px; color:var(--text-dim); line-height:1.6; } .error { color:var(--bad); } .opengoal { display:flex; gap:20px; align-items:center; padding:22px; border-radius:18px; margin-top:18px; } .opengoal img { width:48px; height:48px; object-fit:contain; } .opengoal div { flex:1; } .opengoal p { font-size:13px; }
  @media(max-width:800px) { .tools { padding:22px var(--gutter); } .tool-hero { padding:22px; } .hero-icon { display:none; } .sections { padding:0; border:0; width:100%; } .opengoal { flex-wrap:wrap; } .fields { grid-template-columns:1fr; } }
</style>
