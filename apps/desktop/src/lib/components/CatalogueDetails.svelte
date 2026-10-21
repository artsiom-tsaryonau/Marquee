<script lang="ts">
  import { app } from '../state.svelte';
  import type { GameEntry } from '../types';
  import { platformLabel } from '../labels';
  import { cataloguePlatform, providerScore } from '../metadata';
  let { game }: { game: GameEntry } = $props();
  let selected = $state('');
  let installation = $state('');
  const record = $derived(game.metadata);
  const variants = $derived(record?.media ?? []);
  const variant = $derived(variants.find((m) => m.media_id === (game.source === 'catalogue' ? selected : game.metadataMediaId)) ?? variants[0]);
  const candidates = $derived(app.games.filter((g) => g.source === 'rom' && record && cataloguePlatform(record.system) === g.platform && !g.mediaMissing));
  $effect(() => { if (!variants.some((m) => m.media_id === selected)) selected = variants[0]?.media_id ?? ''; });
  const stale = $derived(app.catalogue.links.some((l) => l.edition_id === game.id && !l.valid));
</script>

{#if record}
  <section class="metadata glass-panel">
    <h3>Source metadata · Vimm’s Lair</h3>
    <p>Saved snapshot. Live availability unknown. {game.source !== 'catalogue' ? `Linked by ${game.metadataMatch === 'sha1' ? 'exact SHA-1' : 'manual confirmation'}.` : 'Catalogue entry only; no local installation is implied.'}</p>
    {#if record.cover}<img class="source-cover" src={record.cover.data_url} alt="{record.title} box cover" loading="lazy" decoding="async" />{/if}
    {#if variants.length}
      {#if game.source === 'catalogue'}<label>Media revision <select data-nav bind:value={selected}>{#each variants as m}<option value={m.media_id}>{m.version || 'Unknown revision'} · {m.filename}</option>{/each}</select></label>{/if}
      <dl>
        <dt>Region</dt><dd>{variant?.regions?.join(', ') || game.region || 'Unknown'}</dd><dt>Languages</dt><dd>{variant?.languages?.join(', ') || game.languages?.join(', ') || 'Unknown'}</dd>
        <dt>Revision</dt><dd>{variant?.version || 'Unknown'}</dd><dt>Media ID</dt><dd>{variant?.media_id}</dd>
        <dt>Filename</dt><dd>{variant?.filename || 'Unknown'}</dd><dt>Advertised size</dt><dd>{variant?.size || 'Unknown'}</dd>
        {#if variant?.serial}<dt>Source serial</dt><dd>{variant.serial}</dd>{/if}
        {#each [['CRC32', variant?.crc32], ['MD5', variant?.md5], ['SHA-1', variant?.sha1]] as [label, value]}{#if value}<dt>{label}</dt><dd>{value}</dd>{/if}{/each}
      </dl>
    {:else}<p>No saved detail page or media variants for this entry.</p>{/if}
    <dl>
      <dt>Players</dt><dd>{record.players ?? 'Unknown'}</dd><dt>Release year</dt><dd>{record.year ?? 'Unknown'}</dd>
      {#each Object.entries(record.ratings) as [label, score]}{#if score !== null}<dt>{label}</dt><dd>{score}/10</dd>{/if}{/each}
      {#if record.rating_votes !== null}<dt>Rating votes</dt><dd>{record.rating_votes}</dd>{/if}
      {#if providerScore(record) !== null && record.ratings.overall === null}<dt>Catalogue rating</dt><dd>{providerScore(record)}/10 · vote count unknown</dd>{/if}
      <dt>Source verified</dt><dd>{variant?.verified_at ?? record.verified_at ?? 'Unknown'} · source claim</dd>
      <dt>Exported</dt><dd>{record.exported_at || 'Unknown'}</dd><dt>Imported</dt><dd>{record.imported_at || 'Unknown'}</dd>
      <dt>Provider game ID</dt><dd>{record.game_id}</dd><dt>Source page</dt><dd>{record.url}</dd>
    </dl>
    {#if record.cover}<p>{record.cover.credit} Attribution: {record.cover.source_url}</p>{/if}
    {#if record.provenance && Object.keys(record.provenance).length}
      <details><summary data-nav>Field provenance</summary>
        <p>Each value retains its source and export date. Export dates do not certify when the source page was captured.</p>
        <dl>{#each Object.entries(record.provenance) as [field, evidence]}
          <dt>{field}</dt><dd>{evidence.source_url}<br />Exported: {evidence.exported_at || 'Unknown'} · Imported: {evidence.imported_at || 'Unknown'}<br />{evidence.method === 'saved_snapshot' ? 'Saved source snapshot' : evidence.method}</dd>
        {/each}</dl>
      </details>
    {/if}
    <button class="btn glass small" data-nav onclick={() => app.openCataloguePage(record.id)}>Open source in browser</button>
    {#if game.source === 'catalogue' && variant && candidates.length}
      <div class="linker"><label>Existing {platformLabel[game.platform]} installation<select data-nav bind:value={installation}><option value="">Choose an installation</option>{#each candidates as candidate}<option value={candidate.id}>{candidate.title} · {candidate.serial ?? candidate.id}</option>{/each}</select></label>
        <p>Confirm this is the same regional revision. Linking changes metadata only; it does not rename files, merge games, move saves or change the launch method.</p>
        <button class="btn glass small" data-nav disabled={!installation || app.catalogueBusy} onclick={() => variant && app.catalogueAction(() => app.api.linkCatalogue(installation, record.id, variant.media_id), 'Metadata linked to installation')}>Confirm media identity & link metadata</button>
      </div>
    {:else if game.source === 'catalogue' && variant}<p>No existing installation for this platform. This entry stays in discovery.</p>
    {:else if game.source === 'rom'}<button class="btn glass small" data-nav disabled={app.catalogueBusy} onclick={() => app.catalogueAction(() => app.api.unlinkCatalogue(game.id), 'Metadata link removed')}>Unlink source metadata</button>{/if}
    {#if app.catalogueError}<p role="alert">{app.catalogueError}</p>{/if}
  </section>
{:else if game.source === 'rom' && app.catalogue.records.length}
  <section class="metadata glass-panel"><h3>Match source metadata</h3>
    <p>{stale ? 'The linked file or source variant changed. Its metadata is no longer applied.' : 'Match this raw ISO against imported SHA-1 values. Archives, CHD and CUE sets need content-aware matching and are not hashed here.'}</p>
    <button class="btn glass small" data-nav disabled={app.catalogueBusy} onclick={() => app.catalogueAction(() => app.api.matchCatalogue(game.id), 'Exact SHA-1 metadata match linked')}>{app.catalogueBusy ? 'Working…' : 'Match raw ISO by SHA-1'}</button>
    {#if stale}<button class="btn glass small" data-nav disabled={app.catalogueBusy} onclick={() => app.catalogueAction(() => app.api.unlinkCatalogue(game.id), 'Stale link removed')}>Remove stale link</button>{/if}
    {#if app.catalogueError}<p role="alert">{app.catalogueError}</p>{/if}
  </section>
{/if}

<style>
  .metadata { padding:22px; border-radius:16px; margin:20px 0; }
  h3 { margin:0 0 12px; } p { color:var(--text-soft); font-size:13px; line-height:1.6; overflow-wrap:anywhere; }
  dl { display:grid; grid-template-columns:130px minmax(0,1fr); gap:10px; font-size:13px; margin:18px 0; }
  dt { color:var(--text-dim); text-transform:capitalize; } dd { margin:0; overflow-wrap:anywhere; }
  label { display:grid; gap:8px; font-size:13px; min-width:0; }
  select { width:100%; min-width:0; padding:10px; border:1px solid var(--line); border-radius:8px; background:var(--field); color:var(--text); font:inherit; }
  .linker { margin-top:22px; } button { margin:4px 6px 4px 0; }
  .source-cover { display:block; width:140px; max-width:100%; height:auto; border-radius:8px; margin:16px 0; }
</style>
