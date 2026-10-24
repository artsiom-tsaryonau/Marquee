<script lang="ts">
  import { untrack } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { MetadataReview } from '../api/contracts';
  import Icon from './Icon.svelte';

  let { editionId, title }: { editionId: string; title: string } = $props();
  let query = $state(untrack(() => title));
  let provider = $state<'details' | 'artwork'>('details');
  let review = $state<MetadataReview | null>(null);
  let busy = $state(false);
  let error = $state('');
  let note = $state('');

  async function search(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true; error = ''; note = ''; review = null;
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
      if (provider === 'details') await app.api.applyGameFacts(editionId, review.token, id);
      else await app.api.applyMetadataMatch(editionId, review.token, id);
      app.applySnapshot(await app.api.snapshot());
      note = provider === 'details'
        ? 'Missing game details filled. Existing details, title, platform, artwork and files were kept.'
        : 'Artwork match saved. Game title, platform and files were kept.';
      review = null;
    } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
</script>

<section class="metadata-search glass-panel">
  <h3>Search metadata and artwork</h3>
  <p>{provider === 'details'
    ? 'Search TheGamesDB for this platform. Preview the release and fill missing details; your existing information is preserved.'
    : 'Find artwork on SteamGridDB. Review the title carefully: platform and edition are not verified by this provider.'}</p>
  <form onsubmit={search}>
    <label class="provider-choice">Search for<select data-nav bind:value={provider} disabled={busy} onchange={() => { review = null; error = ''; note = ''; }}><option value="details">Game details</option><option value="artwork">Artwork</option></select></label>
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
            <small>{candidate.system ? `${candidate.system.toUpperCase()} · ` : ''}{candidate.facts?.year ? `${candidate.facts.year} · ` : ''}Provider ID {candidate.id}</small>
            {#if candidate.facts?.genre || candidate.facts?.players}<p class="facts">{candidate.facts.genre || ''}{candidate.facts.players ? ` · ${candidate.facts.players} player${candidate.facts.players === 1 ? '' : 's'}` : ''}</p>{/if}
            {#if candidate.facts}<details><summary>Preview game details</summary><p>{candidate.facts.description || 'No description supplied for this release.'}</p></details>{/if}
          </div>
          <button class="btn glass small" data-nav disabled={busy} onclick={() => apply(candidate.id)}>{candidate.facts ? 'Fill missing details' : 'Use artwork match'}</button>
        </li>
      {/each}</ul>
    {/if}
  {/if}
</section>

<style>
  .metadata-search { padding: 20px; border-radius: 10px; margin-bottom: 20px; }
  h3 { margin: 0 0 10px; font-size: 18px; }
  p { color: var(--text-soft); font-size: 13px; line-height: 1.6; white-space: pre-line; }
  form { display: flex; align-items: end; gap: 10px; flex-wrap: wrap; }
  label { display: grid; gap: 6px; font-size: 12px; min-width: 0; }
  .query { flex: 1; }
  input, select { font: inherit; color: var(--text); background: var(--field); border: 1px solid var(--line); border-radius: 7px; padding: 10px; min-width: 0; width: 100%; box-sizing: border-box; }
  ul { list-style: none; padding: 0; }
  li { display: flex; justify-content: space-between; align-items: start; gap: 14px; padding: 14px 0; border-top: 1px solid var(--line); }
  .candidate { display: grid; gap: 6px; min-width: 0; flex: 1; }
  strong { font-size: 13px; overflow-wrap: anywhere; }
  small, .provider { color: var(--text-dim); font-size: 11px; }
  .facts { margin: 0; font-size: 12px; }
  details { font-size: 12px; }
  summary { color: var(--text-soft); cursor: pointer; }
  .error { color: var(--danger); }
  @media (max-width: 520px) { li { flex-direction: column; } .query { flex-basis: 100%; } }
</style>
