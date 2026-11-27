<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { FactField, MetadataCandidate, MetadataFactChange, MetadataReview } from '../api/contracts';
  import { factRows } from '../metadataFacts';
  import { systemLabel } from '../labels';
  import Icon from './Icon.svelte';

  let { editionId, title }: { editionId: string; title: string } = $props();
  let query = $state(untrack(() => title));
  let provider = $state<'details' | 'artwork'>('details');
  let review = $state<MetadataReview | null>(null);
  let chosen = $state<MetadataCandidate | null>(null);
  let selectedFields = $state<FactField[]>([]);
  let lastChange = $state<MetadataFactChange | null>(null);
  let reviewHeading = $state<HTMLHeadingElement>();
  let busy = $state(false);
  let error = $state('');
  let note = $state('');
  let undoRevision = 0;
  const rows = $derived(chosen?.facts ? factRows(review?.current, chosen.facts) : []);
  onMount(() => {
    let disposed = false;
    const revision = undoRevision;
    void app.api.gameFactChange(editionId).then(change => {
      if (!disposed && revision === undoRevision) lastChange = change;
    }).catch(e => { if (!disposed && revision === undoRevision) error = asError(e).message; });
    return () => { disposed = true; };
  });

  async function choose(candidate: MetadataCandidate) {
    chosen = candidate;
    selectedFields = factRows(review?.current, candidate.facts ?? {}).filter(row => row.changed && row.missing).map(row => row.field);
    await tick();
    reviewHeading?.focus();
  }
  function toggle(field: FactField) {
    selectedFields = selectedFields.includes(field) ? selectedFields.filter(value => value !== field) : [...selectedFields, field];
  }
  async function saveDetails() {
    if (!review || !chosen || busy) return;
    undoRevision++;
    busy = true; error = ''; note = '';
    try {
      lastChange = await app.api.applyGameFactFields(editionId, review.token, chosen.id, selectedFields);
      review = null; chosen = null;
      note = 'Selected details and release reference saved.';
      app.applySnapshot(await app.api.snapshot());
    } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
  async function undoDetails() {
    if (!lastChange || busy) return;
    undoRevision++;
    busy = true; error = ''; note = '';
    try {
      await app.api.undoGameFactChange(editionId, lastChange.id);
      lastChange = null; review = null; chosen = null;
      note = 'Previous game details and release reference restored.';
      app.applySnapshot(await app.api.snapshot());
      lastChange = await app.api.gameFactChange(editionId);
    } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }

  async function search(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true; error = ''; note = ''; review = null; chosen = null;
    try {
      review = provider === 'details'
        ? await app.api.searchGameFacts(editionId, query)
        : await app.api.searchGameMetadata(editionId, query);
    } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
  async function apply(id: number) {
    if (!review || busy) return;
    busy = true; error = '';
    try {
      await app.api.applyMetadataMatch(editionId, review.token, id);
      app.applySnapshot(await app.api.snapshot());
      note = 'Artwork match saved. Game title, platform and files were kept.';
      review = null;
    } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
</script>

<section class="metadata-search glass-panel">
  <div class="heading"><h3>Search metadata and artwork</h3>{#if lastChange}<button class="btn glass small" data-nav disabled={busy} onclick={undoDetails}>Undo last details change</button>{/if}</div>
  <p>{provider === 'details'
    ? 'Find a release on TheGamesDB, compare its details and choose which fields to update.'
    : 'Find artwork on SteamGridDB. Review the title carefully: platform and edition are not verified by this provider.'}</p>
  <form onsubmit={search}>
    <label class="provider-choice">Search for<select data-nav bind:value={provider} disabled={busy} onchange={() => { review = null; chosen = null; error = ''; note = ''; }}><option value="details">Game details</option><option value="artwork">Artwork</option></select></label>
    <label class="query">Game title<input data-nav bind:value={query} required minlength="2" maxlength="200" disabled={busy} /></label>
    <button class="btn glass small" data-nav disabled={busy || query.trim().length < 2}><Icon name="search" size={16} />{busy ? 'Working…' : 'Search'}</button>
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if note}<p role="status">{note}</p>{/if}
  {#if review}
    <p class="provider">{review.provider}</p>
    {#if !review.candidates.length}<p>No matching candidates. Try another title.</p>
    {:else}
      <ul>{#each review.candidates as candidate}
        <li>
          <div class="candidate">
            <strong>{candidate.title}</strong>
            <small>{candidate.system ? `${systemLabel(candidate.system)} · ` : ''}{candidate.release_date || candidate.facts?.year || 'Release date unknown'} · Provider ID {candidate.id}</small>
            {#if candidate.facts}<p class="release-location">{candidate.country || (candidate.country_id ? `Country ID ${candidate.country_id}` : 'Country unknown')}{candidate.region ? ` · ${candidate.region}` : candidate.region_id ? ` · Region ID ${candidate.region_id}` : ''}</p>{/if}
            {#if candidate.facts?.genre || candidate.facts?.players}<p class="facts">{candidate.facts.genre || ''}{candidate.facts.players ? ` · ${candidate.facts.players} player${candidate.facts.players === 1 ? '' : 's'}` : ''}</p>{/if}
            {#if candidate.facts}<details><summary data-nav>Preview game details</summary><p>{candidate.facts.description || 'No description supplied for this release.'}</p></details>{/if}
          </div>
          <button class="btn glass small" data-nav disabled={busy} aria-pressed={candidate.facts ? chosen?.id === candidate.id : undefined} onclick={() => candidate.facts ? choose(candidate) : apply(candidate.id)}>{candidate.facts ? 'Review details' : 'Use artwork match'}</button>
        </li>
      {/each}</ul>
    {/if}
  {/if}
  {#if chosen && review}
    <section class="field-review" aria-label="Review game details">
      <h3 tabindex="-1" bind:this={reviewHeading}>Review details for {chosen.title}</h3>
      <p class="release-location">{chosen.system ? `${systemLabel(chosen.system)} · ` : ''}{chosen.release_date || chosen.facts?.year || 'Release date unknown'} · {chosen.country || 'Country unknown'}{chosen.region ? ` · ${chosen.region}` : ''} · Provider ID {chosen.id}</p>
      <p>Only checked fields will change. Missing values are selected by default. Saving also uses this release for future image and detail refreshes.</p>
      {#if !rows.length}<p>This release has no details to copy. You can still save its release reference.</p>{/if}
      {#each rows as row}
        <fieldset class="fact-field">
          <legend><label><input type="checkbox" data-nav aria-label={`Use provider ${row.field}`} checked={selectedFields.includes(row.field)} disabled={busy || !row.changed} onchange={() => toggle(row.field)}/>{row.label}{#if !row.changed}<small>Already matches</small>{/if}</label></legend>
          <div class="comparison">
            <div><span>Current</span>{#if row.field === 'description' && row.before}<details><summary data-nav>Read current description</summary><p>{row.before}</p></details>{:else}<p>{row.before == null || row.before === '' ? 'Not set' : row.before}</p>{/if}</div>
            <div><span>TheGamesDB</span>{#if row.field === 'description'}<details><summary data-nav>Read provider description</summary><p>{row.after}</p></details>{:else}<p>{row.after}</p>{/if}</div>
          </div>
        </fieldset>
      {/each}
      <div class="actions"><button class="btn primary small" data-nav disabled={busy} onclick={saveDetails}>{selectedFields.length ? `Save selected details (${selectedFields.length})` : 'Save release reference'}</button><button class="btn glass small" data-nav disabled={busy} onclick={() => chosen = null}>Cancel review</button></div>
    </section>
  {/if}
</section>

<style>
  .metadata-search { padding: 20px; border-radius: 10px; margin-bottom: 20px; min-width: 0; }
  .heading,.actions { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .heading { justify-content: space-between; }
  h3 { margin: 0 0 10px; font-size: 18px; overflow-wrap: anywhere; }
  p { color: var(--text-soft); font-size: 13px; line-height: 1.6; white-space: pre-line; overflow-wrap: anywhere; }
  form { display: flex; align-items: end; gap: 10px; flex-wrap: wrap; }
  label { display: grid; gap: 6px; font-size: 12px; min-width: 0; }
  .query { flex: 1; }
  input:not([type=checkbox]), select { font: inherit; color: var(--text); background: var(--field); border: 1px solid var(--line); border-radius: 7px; padding: 10px; min-width: 0; width: 100%; box-sizing: border-box; }
  ul { list-style: none; padding: 0; }
  li { display: flex; justify-content: space-between; align-items: start; gap: 14px; padding: 14px 0; border-top: 1px solid var(--line); }
  .candidate { display: grid; gap: 6px; min-width: 0; flex: 1; }
  strong { font-size: 13px; overflow-wrap: anywhere; }
  small, .provider { color: var(--text-dim); font-size: 11px; }
  .facts,.release-location { margin: 0; font-size: 12px; }
  details { font-size: 12px; }
  summary { color: var(--text-soft); cursor: pointer; }
  .error { color: var(--danger); }
  .field-review { border-top: 1px solid var(--line); margin-top: 20px; padding-top: 20px; }
  .fact-field { min-width: 0; border: 1px solid var(--line); border-radius: 8px; padding: 12px; margin: 12px 0; }
  legend { max-width: 100%; padding: 0 6px; }
  legend label { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  input[type=checkbox] { accent-color: var(--accent); flex-shrink: 0; }
  .comparison { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 16px; }
  .comparison span { color: var(--text-dim); font-size: 11px; }
  .comparison p { margin: 5px 0; }
  h3:focus-visible,button:focus-visible,summary:focus-visible,input:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  @media (max-width: 600px) { li { flex-direction: column; } .query { flex-basis: 100%; } .comparison { grid-template-columns: minmax(0,1fr); gap: 8px; } }
</style>
