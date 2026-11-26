<script lang="ts">
  import { app } from '../state.svelte';
  import { ago } from '../format';
  import type { XeniaSaves } from '../api/contracts';

  let { editionId }: { editionId: string } = $props();
  let report = $state<XeniaSaves | null>(null);
  let message = $state('');

  $effect(() => {
    const id = editionId;
    report = null;
    message = '';
    void app.api.xeniaSaves(id).then((next) => {
      if (id === editionId) report = next;
    }).catch((error: { message?: string }) => {
      if (id === editionId) message = error.message ?? 'Could not read Xenia save folders';
    });
  });
</script>

<section class="saves">
  <h3>Xbox 360 saves</h3>
  {#if message}
    <p>{message}</p>
  {:else if !report}
    <p>Looking in Xenia’s content folder…</p>
  {:else if !report.title_id}
    <p>This game has no Xbox 360 title id yet, so its save folder cannot be matched.</p>
  {:else if report.folders.length === 0}
    <p>No save folder for {report.title_id} under Xenia’s content directory. A save appears here after the game writes one.</p>
  {:else}
    <p>Title {report.title_id}. These are Xenia’s folders, not a copy of the save.</p>
    <ul>
      {#each report.folders as folder (folder.relative)}
        <li><span>{folder.relative}</span><span>{folder.modified_at === 'unknown' ? 'time unknown' : ago(folder.modified_at)}</span></li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h3 { margin: 0; font-size: 18px; }
  p, li { margin: 0; color: var(--text-soft); font-size: 14px; line-height: 1.5; }
  ul { list-style: none; padding: 0; display: grid; gap: 6px; }
  li { display: flex; justify-content: space-between; gap: 16px; }
  li span:first-child { overflow-wrap: anywhere; }
  .saves { display: grid; gap: 8px; margin-top: 8px; }
</style>
