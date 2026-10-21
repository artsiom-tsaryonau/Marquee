<script lang="ts">
  import { app } from '../state.svelte';
  let expanded = $state(false);
</script>

<div class="catalogue-import glass-panel">
  <button class="btn glass small" data-nav aria-expanded={expanded} onclick={() => expanded = !expanded}>Catalogue metadata · {app.catalogue.records.length} entries</button>
  {#if expanded}
    <p>Import an exported catalogue JSON to add discovery entries, regional revisions and source artwork. Catalogue entries are not installed games.</p>
    <div class="actions">
      <input type="file" accept=".json,application/json" aria-label="Catalogue snapshot JSON" disabled={app.catalogueBusy} onchange={async (event) => { const input = event.currentTarget; const file = input.files?.[0]; if (file) await app.importCatalogueFile(file); input.value = ''; }} />
      {#if app.catalogue.records.length}<button class="btn glass small" data-nav disabled={app.catalogueBusy} onclick={() => app.catalogueAction(() => app.api.clearCatalogue(), 'Catalogue removed; installations and history preserved')}>Remove imported catalogue & metadata links</button>{/if}
    </div>
    <p>{app.catalogue.systems.length} source systems · {app.catalogue.links.filter((l) => l.valid).length} linked installations. Unrecognised platforms remain catalogue-only.</p>
    {#if app.catalogue.systems.length}<details><summary>Source platform information</summary><ul>{#each app.catalogue.systems as system}<li>{system.name} · {system.category} · system launch {system.launch_year || 'unknown'} · {system.slug}</li>{/each}</ul></details>{/if}
    {#if app.catalogueBusy}<p role="status">Working… Checksum matching reads the selected raw ISO and may take a while.</p>{/if}
    {#if app.catalogueError}<p role="alert">{app.catalogueError}</p>{/if}
  {/if}
</div>

<style>
  .catalogue-import { padding:12px; border-radius:14px; margin:12px 0 18px; }
  p { color:var(--text-soft); font-size:13px; line-height:1.6; }
  .actions { display:flex; gap:12px; align-items:center; flex-wrap:wrap; }
  input { max-width:100%; color:var(--text); }
</style>
