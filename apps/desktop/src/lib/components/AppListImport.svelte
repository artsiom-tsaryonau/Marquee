<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { AppListEntry, AppListReview } from '../api/contracts';
  let expanded = $state(false);
  let source = $state('My app list');
  let filename = $state('');
  let original = $state('');
  let review = $state<AppListReview | null>(null);
  let imported = $state<AppListEntry[]>([]);
  let selected = $state<string[]>([]);
  let busy = $state(false);
  let error = $state('');
  let note = $state('');
  const rows = $derived(review?.entries ?? imported);
  const status = (entry: AppListEntry) => entry.status === 'supported_recipe' ? `Managed recipe: ${entry.recipe_id}` : entry.status === 'needs_recipe' ? 'Needs recipe' : 'Unsupported';
  const change = (entry: AppListEntry) => entry.change === 'already_present' ? 'Already present' : entry.change === 'update' ? 'Update discovery details' : 'Add';
  async function run(action: () => Promise<void>) {
    if (busy) return;
    busy = true; error = ''; note = '';
    try { await action(); } catch (e) { error = asError(e).message; }
    finally { busy = false; }
  }
  onMount(() => { void run(async () => { imported = await app.api.appListEntries(); }); });
  async function preview() {
    review = null; selected = [];
    review = await app.api.previewAppList(source, original);
    selected = review.entries.map(e => e.id);
  }
  async function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0]; input.value = '';
    if (!file) return;
    await run(async () => {
      review = null; selected = []; original = ''; filename = '';
      if (file.size > 384 * 1024) throw new Error('Choose a JSON list smaller than 384 KiB.');
      original = await file.text(); filename = file.name;
      await preview();
    });
  }
  async function approve() {
    const current = review;
    if (!current) return;
    await run(async () => {
      const count = await app.api.approveAppList(current.token, selected);
      imported = await app.api.appListEntries(); review = null; selected = [];
      note = `${count} discovery ${count === 1 ? 'entry' : 'entries'} saved. Game files, installations and library preferences were kept.`;
    });
  }
</script>

<section class="app-lists glass-panel" aria-label="App list discovery">
  <button class="btn glass small" data-nav aria-expanded={expanded} onclick={() => expanded = !expanded}>App lists · {imported.length} imported</button>
  {#if expanded}
    <h2>Import an app list</h2>
    <p>Review a local JSON list in Quiver’s apps format. Entries describe projects; importing does not download, install or launch games.</p>
    <div class="inputs">
      <label>Source name<input data-nav bind:value={source} maxlength="120" disabled={busy} oninput={() => { review = null; selected = []; }} /></label>
      <label class="file">Choose JSON list<input data-nav type="file" accept=".json,application/json" aria-label="Quiver app list JSON" disabled={busy || !source.trim()} onchange={choose} /></label>
      {#if original}<button class="btn glass small" data-nav disabled={busy || !source.trim()} onclick={() => run(preview)}>Preview again</button>{/if}
    </div>
    <p class="hint">Reuse the same source name when importing an updated list. Lists are limited to 384 KiB and 500 entries. {filename ? `Selected: ${filename}` : ''}</p>
    {#if busy}<p role="status">Working…</p>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if note}<p role="status">{note}</p>{/if}
    {#if review}
      <div class="review-actions">
        <button class="btn glass small" data-nav disabled={busy || !selected.length} onclick={approve}>Import {selected.length} selected</button>
        <button class="btn glass small" data-nav disabled={busy} onclick={() => selected = selected.length === rows.length ? [] : rows.map(e => e.id)}>{selected.length === rows.length ? 'Deselect all' : 'Select all'}</button>
        <button class="btn glass small" data-nav disabled={busy} onclick={() => { review = null; selected = []; }}>Discard preview</button>
      </div>
      {#each review.diagnostics as diagnostic}<p class="hint">{diagnostic}</p>{/each}
    {/if}
    {#if rows.length}<ul>{#each rows as entry (entry.id)}
      <li>
        {#if review}<input data-nav type="checkbox" aria-label={`Import ${entry.display_name}`} disabled={busy} checked={selected.includes(entry.id)} onchange={(event) => selected = event.currentTarget.checked ? [...selected, entry.id] : selected.filter(id => id !== entry.id)} />{/if}
        <div class="entry">
          <strong>{entry.display_name}</strong>
          {#if entry.project}<span>{entry.project}</span>{/if}
          <small>{entry.provider === 'manual' ? 'Manual project' : `${entry.provider} · ${entry.repository}`} · Source: {entry.source}</small>
          <div class="badges">{#if review}<span>{change(entry)}</span>{/if}<span>{status(entry)}</span>{#each entry.tags as tag}<span>{tag}</span>{/each}</div>
          {#if !review && entry.recipe_id}<div><button class="btn glass small" data-nav disabled={!app.ports.some(p => p.id === entry.recipe_id)} onclick={() => app.portDialogId = entry.recipe_id}>Review managed recipe</button></div>{/if}
          {#if entry.diagnostics.length}<details><summary>Requirements and notes ({entry.diagnostics.length})</summary><ul>{#each entry.diagnostics as diagnostic}<li>{diagnostic}</li>{/each}</ul></details>{/if}
          <details><summary>Original source fields</summary><pre>{JSON.stringify(entry.original, null, 2)}</pre></details>
        </div>
      </li>
    {/each}</ul>{:else}<p class="hint">No entries to display. Choose a list to preview it.</p>{/if}
  {/if}
</section>
<style>
  .app-lists { padding: 14px; border-radius: 10px; margin: 14px 0; }
  h2 { margin: 18px 0 8px; font-size: 18px; } p { color: var(--text-soft); font-size: 13px; line-height: 1.6; }
  .inputs { display: flex; flex-wrap: wrap; gap: 12px; align-items: end; }
  label { display: grid; gap: 6px; font-size: 12px; min-width: 0; } label > input:not([type=file]) { padding: 9px; border: 1px solid var(--line); border-radius: 7px; font: inherit; color: var(--text); background: var(--field); }
  .file { flex: 1; min-width: 200px; } input[type=file] { max-width: 100%; font: inherit; color: var(--text-soft); }
  .review-actions, .badges { display: flex; gap: 8px; flex-wrap: wrap; } .review-actions { margin: 16px 0; }
  ul { list-style: none; padding: 0; } .app-lists > ul > li { display: flex; align-items: start; gap: 12px; border-top: 1px solid var(--line); padding: 16px 0; }
  .entry { display: grid; gap: 7px; min-width: 0; flex: 1; overflow-wrap: anywhere; } strong { font-size: 14px; } .entry > span, details { font-size: 12px; }
  small, .hint { color: var(--text-dim); font-size: 11px; } .badges > span { padding: 4px 7px; font-size: 11px; border: 1px solid var(--line); border-radius: 5px; }
  summary { cursor: pointer; color: var(--text-soft); } details li { padding: 5px 0; color: var(--text-soft); } pre { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 11px; color: var(--text-soft); }
  .error { color: var(--danger); } input[type=checkbox] { margin-top: 3px; }
</style>
