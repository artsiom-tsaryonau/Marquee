<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { asError } from '../api/core';
  import type { StorageSnapshot } from '../api/contracts';
  import { bytes } from '../format';
  import Icon from './Icon.svelte';
  import StorageMove from './StorageMove.svelte';

  const groups = [
    ['library', 'Games and library'], ['sources', 'Originals and acquisition'],
    ['programs', 'Emulator programs'], ['firmware', 'BIOS and firmware'],
    ['user-data', 'Saves and profiles'], ['media', 'Music, images and video'],
    ['streaming', 'Streaming'],
  ] as const;
  let snapshot = $state<StorageSnapshot | null>(null);
  let filter = $state('all');
  let busy = $state(false);
  let loading = $state(false);
  let error = $state('');
  let notice = $state('');
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function refresh() {
    if (loading || disposed) return;
    loading = true;
    clearTimeout(timer);
    try {
      const result = await app.api.storageSnapshot();
      if (disposed) return;
      snapshot = result;
      if (result.scanning) timer = setTimeout(() => void refresh(), 800);
    } catch (e) { if (!disposed) error = asError(e).message; }
    finally { if (!disposed) loading = false; }
  }
  async function scan() {
    if (busy || loading || snapshot?.scanning) return;
    busy = true; error = ''; notice = '';
    try { await app.api.storageScan(); await refresh(); }
    catch (e) { if (!disposed) error = asError(e).message; }
    finally { if (!disposed) busy = false; }
  }
  async function folder(kind: 'collection' | 'bios_inbox', clear = false) {
    if (busy || loading || snapshot?.scanning) return;
    busy = true; error = ''; notice = '';
    try {
      const path = clear ? null : await app.api.pickRoot(kind === 'collection' ? 'Choose the Marquee collection folder' : 'Choose an additional BIOS import folder');
      if (!clear && !path) return;
      await app.api.storageSetFolder(kind, path);
      if (disposed) return;
      notice = 'Folder setting saved. Existing files, game paths, firmware and saves stay in place.';
      await refresh();
    } catch (e) { if (!disposed) error = asError(e).message; }
    finally { if (!disposed) busy = false; }
  }
  async function open(id: string) {
    error = '';
    try { await app.api.storageOpen(id); }
    catch (e) { if (!disposed) error = asError(e).message; }
  }
  onMount(() => {
    void refresh();
    return () => { disposed = true; clearTimeout(timer); };
  });
</script>

<div class="storage-panel">
  <header>
    <div><h2>Storage</h2><p>Know where your games, originals and saves live.</p></div>
    <button class="btn glass small" data-nav onclick={scan} disabled={busy || loading || snapshot?.scanning || !snapshot}>
      <Icon name="refresh" size={15} />{snapshot?.scanning ? 'Measuring…' : 'Measure usage'}
    </button>
  </header>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if !snapshot}<p role="status">{error ? 'Storage locations could not be loaded.' : 'Loading locations…'}</p>{#if error}<button class="btn glass small" data-nav onclick={refresh}>Retry</button>{/if}
  {:else}
    <section class="glass-panel destinations" aria-label="Storage folder settings">
      <div class="destination">
        <div><h3>Collection folder</h3><code>{snapshot.collection_root || 'Use existing library folders'}</code>
          <p>For immediate archive imports, an existing platform folder takes precedence. New platform folders go under <code>library/imported-roms</code> here. Originals are shown under <code>sources</code>.</p>
          <p>Manager-prepared installations keep their receipt-owned location. Choosing a folder does not move or scan existing files.</p></div>
        <div class="actions"><button class="btn glass small" data-nav disabled={busy || loading || snapshot.scanning} onclick={() => folder('collection')}><Icon name="folder" size={15} />Choose</button>
          {#if snapshot.collection_root}<button class="btn glass small" data-nav disabled={busy || loading || snapshot.scanning} onclick={() => folder('collection', true)}>Reset</button>{/if}</div>
      </div>
      <div class="destination">
        <div><h3>Additional BIOS import folder</h3><code>{snapshot.bios_inbox || 'Only the built-in bios-inbox folder'}</code>
          <p>Recognized files are copied into emulator profiles at launch. Installed firmware and saves remain separate from this input folder.</p></div>
        <div class="actions"><button class="btn glass small" data-nav disabled={busy || loading || snapshot.scanning} onclick={() => folder('bios_inbox')}><Icon name="folder" size={15} />Choose</button>
          {#if snapshot.bios_inbox}<button class="btn glass small" data-nav disabled={busy || loading || snapshot.scanning} onclick={() => folder('bios_inbox', true)}>Reset</button>{/if}</div>
      </div>
    </section>
    <StorageMove onchange={() => void refresh()} />
    <nav class="filters" aria-label="Storage categories">
      <button data-nav class:on={filter === 'all'} aria-pressed={filter === 'all'} onclick={() => filter = 'all'}>All locations</button>
      {#each groups as [id, label]}<button data-nav class:on={filter === id} aria-pressed={filter === id} onclick={() => filter = id}>{label}</button>{/each}
    </nav>
    <p class="measurement" role="status">{snapshot.scanning ? 'Measuring file sizes in the background. You can leave this page. ' : snapshot.measured_at ? `Measured ${new Date(snapshot.measured_at).toLocaleString()} · ` : 'Usage is measured only when requested. '}Sizes are logical file sizes. Locations can overlap; do not add the rows together. Nested links and special files are skipped.</p>
    {#each groups.filter(([id]) => filter === 'all' || filter === id) as [id, label]}
      {@const rows = snapshot.locations.filter(row => row.category === id)}
      {#if rows.length}
        <section class="location-group" aria-label={label}>
          <h3>{label}</h3>
          <div class="locations glass-panel">
            {#each rows as row (row.id)}
              <article>
                <div class="location-info">
                  <div class="location-title"><strong>{row.label}</strong>{#if !row.exists}<span class="badge">Not present</span>{/if}</div>
                  <code>{row.path}</code>
                  {#if row.resolved && row.resolved !== row.path}<span class="resolved">Physical location: <code>{row.resolved}</code></span>{/if}
                  <p>{row.note}</p>
                </div>
                <div class="usage">
                  <strong>{row.usage ? bytes(row.usage.bytes) : row.exists ? '—' : 'Not present'}</strong>
                  {#if row.usage}<span>{row.usage.files.toLocaleString()} {row.usage.files === 1 ? 'file' : 'files'}{row.usage.incomplete ? ' · partial' : ''}</span>{#if row.usage.skipped_links}<span>{row.usage.skipped_links} links skipped</span>{/if}{#if row.usage.skipped_special}<span>{row.usage.skipped_special} special files skipped</span>{/if}{/if}
                  {#if row.free_bytes !== null}<span>{bytes(row.free_bytes)} free on volume</span>{/if}
                  <button class="btn glass small" data-nav disabled={!row.exists} onclick={() => open(row.id)} aria-label={`Open ${row.label} folder`}><Icon name="folder" size={14} />Open folder</button>
                </div>
              </article>
            {/each}
          </div>
        </section>
      {/if}
    {/each}
    <aside class="move-note glass-panel"><h3>Other storage moves</h3><p>The offline relocation tool supports reviewed source and ROM moves. Emulator profiles, firmware and streaming storage still require separate coordinated migrations. Keep their paths and existing compatibility links intact.</p></aside>
  {/if}
</div>

<style>
  .storage-panel { display: grid; gap: 20px; min-width: 0; }
  header, .destination { display: flex; align-items: start; justify-content: space-between; gap: 18px; }
  header h2 { margin: 0; font-size: 24px; }
  h3 { margin: 0; font-size: 14px; }
  p { color: var(--text-dim); font-size: 12px; line-height: 1.65; margin: 6px 0 0; }
  code { color: var(--text-soft); font-size: 11px; overflow-wrap: anywhere; white-space: normal; }
  .destinations, .move-note { padding: 20px; border-radius: 12px; }
  .destination + .destination { margin-top: 20px; padding-top: 20px; border-top: 1px solid var(--line); }
  .destination > div:first-child { display: grid; gap: 5px; min-width: 0; }
  .actions { display: flex; gap: 6px; flex-shrink: 0; }
  .filters { display: flex; gap: 7px; flex-wrap: wrap; }
  .filters button { border: 1px solid var(--line); background: var(--field); color: var(--text-soft); padding: 8px 10px; border-radius: 7px; font: inherit; font-size: 11px; cursor: pointer; }
  .filters button.on { color: var(--text); border-color: var(--accent); background: var(--field-strong); }
  .measurement { margin: 0; }
  .location-group { display: grid; gap: 10px; min-width: 0; }
  .locations { border-radius: 12px; overflow: hidden; }
  article { display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 20px; padding: 18px; }
  article + article { border-top: 1px solid var(--line); }
  .location-info { min-width: 0; display: grid; gap: 7px; align-content: start; }
  .location-title { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .location-title strong { font-size: 13px; }
  .badge, .resolved { color: var(--text-dim); font-size: 10px; }
  .badge { padding: 3px 6px; background: var(--field); border-radius: 4px; }
  .location-info p { margin: 0; }
  .usage { display: grid; gap: 5px; justify-items: end; align-content: start; text-align: right; }
  .usage strong { font-size: 14px; }
  .usage span { font-size: 10px; color: var(--text-dim); }
  .usage button { margin-top: 6px; }
  .error { color: var(--danger); }
  .notice { color: var(--ok); }
  @media(max-width:600px) { header, .destination { flex-wrap: wrap; } article { grid-template-columns: 1fr; gap: 12px; } .usage { justify-items: start; text-align: left; } }
</style>
