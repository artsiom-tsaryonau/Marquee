<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { artFromSeed } from '../art';
  import { coverSrc } from '../api/map';
  import { systemLabel } from '../labels';
  import Artwork from './Artwork.svelte';
  import Icon from './Icon.svelte';
  import type { EmulatorBuildStatus } from '../api/contracts';
  import { pollWhileActive } from '../activity';

  const emulator = $derived(app.emulators.find((e) => e.id === app.emulatorDialogId) ?? null);
  const sourceId = $derived(emulator?.source_build ? emulator.id : null);
  const dur = $derived(app.motionReduced ? 0 : 240);

  let args = $state<string[]>([]);
  let loadedFor = $state<string | null>(null);
  let error = $state('');
  let saving = $state(false);
  let confirmPurge = $state(false);
  let buildStatus = $state<EmulatorBuildStatus | null>(null);
  let startingBuild = $state(false);
  const busy = $derived(buildStatus?.state === 'running' ? buildStatus.message : emulator ? app.emulatorBusy[emulator.id] : undefined);

  // Backend jobs survive closing the dialog or refreshing the browser. Polling
  // restores their status and a bounded log tail without tying jobs to a tab.
  $effect(() => {
    const id = sourceId;
    buildStatus = null;
    if (!id) return;
    let disposed = false;
    let polling = false;
    let lastState: string | undefined;
    async function poll() {
      if (polling || !id) return;
      polling = true;
      try {
        const status = await app.api.emulatorBuildStatus(id);
        if (disposed) return;
        if (JSON.stringify(buildStatus) !== JSON.stringify(status)) buildStatus = status;
        if (status && status.state !== 'running' && lastState !== status.state) await app.loadEmulators();
        lastState = status?.state;
      } catch (err) {
        if (!disposed) error = (err as Error).message;
      } finally { polling = false; }
    }
    const stop = pollWhileActive(poll, () => buildStatus?.state === 'running' ? 2000 : 15000);
    return () => { disposed = true; stop(); };
  });

  async function build() {
    if (!emulator || startingBuild) return;
    startingBuild = true;
    try {
      await app.buildEmulator(emulator);
      buildStatus = await app.api.emulatorBuildStatus(emulator.id);
    } catch (err) { error = (err as Error).message; }
    finally { startingBuild = false; }
  }

  async function cancelBuild() {
    if (!emulator) return;
    try { await app.api.cancelEmulatorBuild(emulator.id); }
    catch (err) { error = (err as Error).message; }
  }

  async function chooseGames() {
    try {
      const folder = await app.api.pickRoot();
      if (!folder) return;
      args = [...args.filter((a) => !a.startsWith('--marquee-games=')), `--marquee-games=${folder}`];
      await save();
    } catch (err) { error = (err as Error).message; }
  }

  // Start from what is saved each time a different emulator opens.
  $effect(() => {
    if (emulator && loadedFor !== emulator.id) {
      loadedFor = emulator.id;
      args = [...emulator.args];
      error = '';
      confirmPurge = false;
    } else if (!emulator) {
      loadedFor = null;
    }
  });

  async function save() {
    if (!emulator || saving) return;
    saving = true;
    error = '';
    try {
      if (await app.configureEmulator(emulator, args)) args = [...(app.emulators.find((e) => e.id === emulator.id)?.args ?? [])];
    } catch (err) {
      error = (err as Error).message;
    } finally {
      saving = false;
    }
  }
  async function remove(purge: boolean) {
    if (!emulator) return;
    if (purge && !confirmPurge) {
      confirmPurge = true;
      return;
    }
    confirmPurge = false;
    await app.uninstallEmulator(emulator, purge);
  }
</script>

{#if emulator}
  {@const e = emulator}
  {@const icon = coverSrc(e.icon)}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={() => (app.emulatorDialogId = null)} role="presentation"></div>
  <div class="dialog glass-panel" role="dialog" aria-modal="true" aria-labelledby="emu-title" transition:fly={{ y: 24, duration: dur }}>
    <div class="banner">
      <Artwork art={artFromSeed(`emulator:${e.id}`)} />
      <div class="shade"></div>
      {#if icon}<img class="icon" src={icon} alt="" />{:else}<span class="icon fallback"><Icon name="controller" size={34} /></span>{/if}
      <button class="close btn glass small" data-nav onclick={() => (app.emulatorDialogId = null)} aria-label="Close"><Icon name="x" /></button>
      <div class="titles">
        <h2 id="emu-title">{e.name}</h2>
        <p>{e.systems.map(systemLabel).join(' · ')}</p>
      </div>
    </div>

    <div class="body">
      <p class="lead">{e.requirement}</p>

      <div class="status">
        <span class="badge" class:ok={e.status === 'ready'} class:warn={e.status === 'update'}>
          {e.status === 'not_installed' ? 'Not installed' : e.status === 'update' ? 'Update available' : 'Ready'}
        </span>
        {#if e.installed_version}<span class="ver">Version {e.installed_version.length > 24 ? `${e.installed_version.slice(0, 20)}…` : e.installed_version}</span>{/if}
        {#if busy}<span class="busy" role="status">{busy}</span>{/if}
        <span class="spacer"></span>
        {#if e.status === 'not_installed'}
          <button class="btn primary small" data-nav disabled={!!busy || startingBuild} onclick={() => e.source_build ? build() : app.installEmulator(e)}><Icon name="download" size={15} />{e.source_build ? 'Build from source' : 'Install'}</button>
        {:else}
          <button class="btn primary small" data-nav disabled={!!busy} onclick={() => app.openEmulator(e)}><Icon name="play" size={15} />Open</button>
          {#if e.status === 'update'}<button class="btn glass small" data-nav disabled={!!busy} onclick={() => e.source_build ? build() : app.installEmulator(e)}>Update</button>{/if}
        {/if}
      </div>

      {#if e.source_build}
        <section>
          <h3>Managed source build</h3>
          <p class="hint">{e.name} · pinned source · Linux x86_64 · rootless Podman + Git. Compiler and runtime dependencies stay in a container. First build downloads dependencies and may take several minutes. Selected game media is mounted read-only; saves use Marquee’s writable folders.</p>
          {#if e.id === 'mgba'}<button class="btn glass small" data-nav disabled={!!busy || saving} onclick={chooseGames}>Choose game folder</button>{/if}
          {#if args.some((a) => a.startsWith('--marquee-games='))}<p class="hint">{args.find((a) => a.startsWith('--marquee-games='))?.slice('--marquee-games='.length)}</p>{/if}
          <div class="arg-actions">
            {#if busy}<button class="btn glass small" data-nav onclick={cancelBuild}>Cancel build</button>
            {:else if e.status !== 'not_installed' || buildStatus}
              <button class="btn glass small" data-nav disabled={startingBuild} onclick={build}>{buildStatus?.state === 'failed' || buildStatus?.state === 'interrupted' || buildStatus?.state === 'cancelled' ? 'Retry build' : 'Rebuild pinned version'}</button>
            {/if}
            {#if e.can_rollback}<button class="btn glass small" data-nav disabled={!!busy} onclick={() => app.rollbackEmulator(e)}>Restore previous version</button>{/if}
          </div>
          {#if buildStatus}
            <p class="hint" role="status">{buildStatus.state} · {buildStatus.phase}: {buildStatus.message}</p>
            <details class="alt"><summary>Build log</summary><p class="hint">{buildStatus.log_path}</p><pre>{buildStatus.log_tail || 'Waiting for output…'}</pre></details>
          {/if}
          {#if e.source_receipt}
            <details class="alt"><summary>Build provenance</summary><p class="hint">Commit: {e.source_receipt.commit}<br />Image: {e.source_receipt.image}<br />{e.source_receipt.validation}</p></details>
          {/if}
        </section>
      {/if}

      {#if e.launch_presentation}<p class="hint">{e.launch_presentation}</p>{/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}

      {#if (e.firmware ?? []).length}
        <section>
          <h3>Firmware</h3>
          <p class="hint">Put the files in Marquee’s bios-inbox folder; they are copied into place on the next launch.</p>
          <ul class="fw">
            {#each e.firmware ?? [] as f (f.name)}
              <li class:missing={!f.present}><strong>{f.name}</strong> <span>{f.present ? 'in place' : f.required ? 'missing (required)' : 'missing (optional)'}</span>{#if f.note}<small>{f.note}</small>{/if}</li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if e.status !== 'not_installed'}
        <section>
          <h3>Where things live</h3>
          <dl>
            {#if e.program_dir}<dt>Program</dt><dd>{e.program_dir}</dd>{/if}
            <dt>Settings and saves</dt><dd>{e.data_dir}</dd>
            {#if e.bios_dir}<dt>BIOS</dt><dd>{e.bios_dir}</dd>{/if}
          </dl>
        </section>
      {/if}

      {#if e.status !== 'not_installed'}
        <section class="danger-zone">
          <h3>Remove</h3>
          <p class="hint">Removing the program keeps your settings and saves, so reinstalling picks up where you left off.</p>
          <div class="arg-actions">
            <button class="btn glass small" data-nav disabled={!!busy} onclick={() => remove(false)}>Remove {e.name}</button>
            <button class="btn glass small danger" data-nav disabled={!!busy} onclick={() => remove(true)} onblur={() => (confirmPurge = false)}>
              {confirmPurge ? 'Click again: delete saves too' : 'Remove with saves…'}
            </button>
          </div>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; z-index: 40; background: rgb(0 0 0 / .6); backdrop-filter: var(--overlay-filter); }
  .dialog { position: fixed; z-index: 41; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(680px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow: auto; border-radius: 22px; padding: 0; }
  .banner { position: relative; height: 170px; overflow: hidden; border-radius: 22px 22px 0 0; }
  .banner :global(svg) { width: 100%; height: 100%; object-fit: cover; }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, rgb(8 8 14 / .85), transparent 70%); }
  .icon { position: absolute; top: 24px; left: 28px; width: 84px; height: 84px; object-fit: contain; }
  :global(:root[data-effects='full']) .icon { filter: drop-shadow(0 8px 18px rgb(0 0 0 / .5)); }
  .icon.fallback { display: grid; place-items: center; border-radius: 22px; background: rgb(0 0 0 / .35); color: #fff; }
  .close { position: absolute; top: 16px; right: 16px; }
  .titles { position: absolute; left: 28px; right: 28px; bottom: 16px; }
  .titles h2 { margin: 0; font-size: 26px; letter-spacing: -.5px; }
  .titles p { margin: 3px 0 0; font-size: 13px; color: var(--text-soft); }
  .body { padding: 22px 28px 28px; display: grid; gap: 20px; }
  p { margin: 0; line-height: 1.6; font-size: 13px; }
  .lead { color: var(--text-soft); }
  .hint { color: var(--text-dim); font-size: 12px; }
  h3 { margin: 0 0 6px; font-size: 14px; }
  .status { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .spacer { flex: 1; }
  .badge { padding: 4px 8px; border-radius: 6px; font-size: 11px; color: var(--text-dim); background: var(--field); }
  .badge.ok { color: var(--ok); background: rgb(52 211 153 / .08); }
  .badge.warn { color: var(--warn); background: rgb(251 191 36 / .1); }
  .ver { font-size: 12px; color: var(--text-dim); }
  .busy { font-size: 12px; color: var(--accent); }
  .arg-actions { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; justify-content: space-between; margin-top: 4px; }
  .error { color: var(--bad); font-size: 12px; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 14px; margin: 8px 0 0; font-size: 12px; }
  dt { color: var(--text-dim); }
  dd { margin: 0; font: 11px ui-monospace, monospace; overflow-wrap: anywhere; color: var(--text-soft); }
  .alt { font-size: 12px; color: var(--text-dim); }
  .alt summary { cursor: pointer; }
  .fw { list-style: none; margin: 8px 0 0; padding: 0; display: grid; gap: 8px; }
  .fw li { display: grid; gap: 2px; }
  .fw .missing span { color: var(--warn, #e0a030); }
  .fw small { color: var(--text-soft); }
  .danger-zone { border-top: 1px solid var(--line); padding-top: 16px; }
  pre { max-height: 240px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; font: 11px ui-monospace, monospace; background: var(--field); padding: 10px; border-radius: 8px; }
  .btn.danger:hover, .btn.danger:focus-visible { color: var(--bad); }
</style>
