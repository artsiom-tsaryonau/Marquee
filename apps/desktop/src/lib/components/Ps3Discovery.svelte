<script lang="ts">
  import { app } from '../state.svelte';
  import { bytes } from '../format';
  import type { Ps3Inspection } from '../api/contracts';
  import Icon from './Icon.svelte';
  let added=$state('');
  async function add(path:string){busy=true;error='';try{const id=await app.api.importPs3Disc(path);app.applySnapshot(await app.api.snapshot());added=id;}catch(e){error=(e as Error).message;}finally{busy=false;}}
  let busy = $state(false);
  let error = $state('');
  let report = $state<Ps3Inspection | null>(null);
  const names: Record<string, string> = { archive: 'Archive candidate', disc_folder: 'Disc folder', digital_folder: 'Digital game folder candidate', game_data: 'Game data / update candidate', other_content: 'Other content', disc_image: 'Disc image candidate', package: 'Package candidate', firmware: 'Firmware candidate', archived_metadata: 'Archived metadata' };
  async function inspect(folder: boolean) {
    if (busy) return;
    busy = true; error = '';
    try {
      const path = await (folder ? app.api.pickRoot() : app.api.pickPs3Content());
      if (path) { report = null; report = await app.api.inspectPs3(path); }
    } catch (err) { error = (err as Error).message; }
    finally { busy = false; }
  }
</script>

<section class="discovery">
  <h3>PS3 content discovery</h3>
  <p>Review archives or game folders before preparation. This check reads metadata and archive headers; it does not extract, install or add games.</p>
  <div class="actions"><button class="btn glass small" data-nav disabled={busy} onclick={() => inspect(false)}><Icon name="search" size={16} />Inspect a file</button><button class="btn glass small" data-nav disabled={busy} onclick={() => inspect(true)}><Icon name="folder" size={16} />Inspect a folder</button></div>
  {#if added}<p role="status">Added to Games. Open its Media tab to link an extras collection.</p>{/if}
  {#if busy}<p role="status">Inspecting content…</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if report}
    <div class="result" aria-live="polite">
      <p class="path">{report.source}</p>
      <p>{report.source_kind === 'archive' ? 'Archive headers' : 'Local content'} · {report.inspected_entries} entries checked{report.source_bytes !== null ? ` · ${bytes(report.source_bytes)}` : ''}</p>
      {#if report.archive_unpacked_bytes !== null}<p>Declared unpacked size: {bytes(report.archive_unpacked_bytes)}. This is not a free-space or extraction estimate.</p>{/if}
      {#each report.items as item}
        <article>
          <h4>{item.metadata?.title ?? names[item.kind] ?? 'Unidentified content'}</h4>
          <p class="path">{item.path}</p>
          {#if item.metadata}<dl><dt>Title ID</dt><dd>{item.metadata.title_id}</dd><dt>Content category</dt><dd>{item.metadata.category} · {names[item.kind]}</dd>{#if item.metadata.app_version}<dt>Version</dt><dd>{item.metadata.app_version}</dd>{/if}<dt>Boot file</dt><dd>{item.boot_file_present ? 'Present; not launched' : 'Not found in this layout'}</dd><dt>Artwork</dt><dd>{item.artwork_present ? 'ICON0.PNG present; not decoded' : 'Not found'}</dd></dl>{/if}
          {#if item.bytes !== null}<p>{bytes(item.bytes)}</p>{/if}
          {#if item.kind==='disc_image'&&report.source_kind!=='archive'}<button class="btn glass small" data-nav disabled={busy} onclick={()=>add(item.path)}>Add disc to Games</button><p>Registers a verified disc for RPCS3. Install firmware through Firmware setup before playing.</p>{/if}
          <p class="status">{item.status}</p><p>{item.next_action}</p>
        </article>
      {/each}
      {#each report.diagnostics as diagnostic}<p class="diagnostic">{diagnostic}</p>{/each}
    </div>
  {/if}
</section>

<style>
  .discovery { border-top: 1px solid var(--line); padding-top: 18px; margin-top: 18px; }
  h3, h4 { margin: 0 0 8px; } h3 { font-size: 15px; } h4 { font-size: 14px; }
  p { color: var(--text-soft); font-size: 13px; line-height: 1.6; margin: 8px 0; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .result { margin-top: 16px; }
  article { padding: 14px; margin-top: 12px; background: var(--field); border: 1px solid var(--line); border-radius: 10px; }
  .path { overflow-wrap: anywhere; color: var(--text-dim); font-size: 12px; }
  .status { color: var(--text); font-weight: 600; }
  .error { color: var(--danger); } .diagnostic { color: var(--text-dim); }
  dl { display: grid; grid-template-columns: 110px minmax(0, 1fr); gap: 6px 12px; font-size: 12px; }
  dt { color: var(--text-dim); } dd { margin: 0; overflow-wrap: anywhere; }
</style>
