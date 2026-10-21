<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { modal } from '../modal';
  import { systemLabel } from '../labels';
  import Icon from './Icon.svelte';
  let draft = $state('');
  let error = $state('');
  let listening = true;
  async function browse() {
    try { app.queueArchives(await app.api.pickArchives()); }
    catch (err) { error = (err as Error).message; }
  }
  function addPaths() { app.queueArchives(draft.split('\n')); draft = ''; }
  function browserDrop(event: DragEvent) {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    if (app.api.kind === 'web') {
      app.intakeOpen = true;
      error = 'Browser files cannot supply a path on the Marquee computer. Enter that computer’s absolute archive path below.';
    }
  }
  onMount(() => {
    let stop = () => {};
    void app.api.onArchiveDrop((paths) => { if (listening) app.queueArchives(paths); }).then((unlisten) => {
      if (!listening) unlisten(); else stop = unlisten;
    }).catch(() => { /* Native file picking remains available if drag/drop is unavailable. */ });
    return () => { listening = false; stop(); };
  });
  const outcome = { imported:'Files imported', kept_archive:'Arcade archive copied', stored_no_emulator:'Stored · adapter unavailable', already_present:'Already present', skipped:'Skipped' };
</script>

<svelte:window ondragover={(event) => { if (event.dataTransfer?.types.includes('Files')) event.preventDefault(); }} ondrop={browserDrop} />

{#if app.intakeOpen}
  <div class="scrim" data-modal-dismiss role="presentation" onclick={() => (app.intakeOpen = false)}></div>
  <div class="dialog glass-panel" role="dialog" aria-modal="true" aria-labelledby="archive-title" use:modal={() => (app.intakeOpen = false)}>
    <header><div><h2 id="archive-title">Import game archives</h2><p>ZIP and 7z archives are checked, extracted into library folders, then scanned. RAR is not supported.</p></div><button class="btn glass small" data-nav aria-label="Close archive import" onclick={() => (app.intakeOpen = false)}><Icon name="x" /></button></header>
    <div class="body">
      {#if app.api.kind === 'desktop'}<p>Choose archives or drop them onto Library. Review the selection before importing.</p>{:else}<p>Enter paths on the computer running Marquee. Browser uploads and files on another computer are not supported.</p>{/if}
      <form onsubmit={(event) => { event.preventDefault(); addPaths(); }}>
        <label>Archive paths · one per line<textarea data-nav rows="3" placeholder="/home/you/Downloads/Game.zip" bind:value={draft} disabled={app.intakeBusy}></textarea></label>
        <div class="actions"><button class="btn glass small" data-nav type="button" disabled={app.intakeBusy} onclick={browse}><Icon name="folder" size={16} />{app.api.kind === 'desktop' ? 'Choose archives' : 'Enter a host path'}</button><button class="btn glass small" data-nav disabled={app.intakeBusy || !draft.trim()}>Add paths</button></div>
      </form>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      {#if app.intakePaths.length}
        <h3>Selected archives · {app.intakePaths.length}</h3>
        <ul class="selection">{#each app.intakePaths as path (path)}<li><span>{path}</span><button class="btn glass small" data-nav disabled={app.intakeBusy} aria-label={`Remove ${path}`} onclick={() => (app.intakePaths = app.intakePaths.filter((p) => p !== path))}><Icon name="x" size={14} /></button></li>{/each}</ul>
        <label class="check"><input type="checkbox" data-nav bind:checked={app.intakeDelete} disabled={app.intakeBusy} /> Remove source archives after successful extraction and size checks</label>
        <p>Original archives are kept by default. System detection uses file types and disc size; review the detected system below. Firmware archives belong in Tools → Import BIOS.</p>
        <button class="btn primary" data-nav disabled={app.intakeBusy} onclick={() => app.importArchives(app.intakePaths, app.intakeDelete)}>{app.intakeBusy ? 'Importing…' : app.intakeResults.some((r) => r.error) ? 'Retry selected archives' : 'Import selected archives'}</button>
      {/if}
      {#if app.intakeProgress}<p role="status">{app.intakeProgress}</p>{/if}
      {#if app.intakeBusy}<p>Working on the Marquee computer. Byte progress and cancellation are not available for this operation. You can close this panel and check back in Library.</p>{/if}
      {#if app.intakeScanError}<div class="scan-error"><p class="error" role="alert">Files were processed, but the library scan or refresh failed: {app.intakeScanError}</p><button class="btn glass small" data-nav disabled={app.intakeBusy} onclick={() => app.refreshImportedGames()}>Retry library scan</button></div>{/if}
      {#each app.intakeResults as result, index (`${index}:${result.path}`)}
        <article class="result" class:failed={!!result.error}><h3>{result.path.split('/').at(-1)}</h3>
          {#if result.error}<p class="error" role="alert">{result.error}</p>{:else if result.report}
            <p><strong>{outcome[result.report.outcome]}</strong>{result.report.system ? ` · ${systemLabel(result.report.system)}` : ''}</p>
            <p>{result.report.message}</p><p>{result.report.source_removed ? 'Source archive removed' : 'Source archive kept'}</p>
            {#if result.report.dest}<p class="path">Destination: {result.report.dest}</p>{/if}
            {#if result.report.files.length}<details><summary data-nav>Files · {result.report.files.length}</summary><ul>{#each result.report.files.slice(0,100) as file}<li>{file}</li>{/each}</ul>{#if result.report.files.length > 100}<p>Showing the first 100 filenames.</p>{/if}</details>{/if}
          {/if}
        </article>
      {/each}
    </div>
  </div>
{/if}

<style>
  .scrim { position:fixed; inset:0; z-index:40; background:rgb(0 0 0 / .65); backdrop-filter:var(--overlay-filter); }
  .dialog { position:fixed; top:5vh; left:50%; transform:translateX(-50%); width:min(800px,calc(100% - 32px)); max-height:90vh; overflow:auto; z-index:41; border-radius:20px; background:var(--surface-strong); }
  header { display:flex; gap:20px; align-items:start; padding:24px; border-bottom:1px solid var(--line); } header .btn { margin-left:auto; }
  h2 { margin:0 0 10px; font-size:26px; } h3 { margin:0; font-size:16px; overflow-wrap:anywhere; }
  p { margin:0; color:var(--text-soft); font-size:13px; line-height:1.6; } .body { display:grid; gap:16px; padding:24px; }
  form, label { display:grid; gap:10px; font-size:14px; } textarea { font:inherit; resize:vertical; background:var(--field); border:1px solid var(--line); color:var(--text); border-radius:10px; padding:12px; width:100%; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; } .check { display:flex; gap:10px; align-items:start; }
  .selection { display:grid; gap:8px; list-style:none; margin:0; padding:0; max-height:220px; overflow:auto; } .selection li { display:flex; align-items:center; gap:10px; } .selection span { flex:1; overflow-wrap:anywhere; font-size:13px; color:var(--text-soft); }
  .result { padding:16px; border:1px solid var(--line); border-radius:12px; display:grid; gap:10px; } .result.failed { border-color:var(--bad); }
  .error { color:var(--bad); } .path, li { overflow-wrap:anywhere; } details { font-size:13px; } summary { cursor:pointer; padding:8px 0; }
</style>
