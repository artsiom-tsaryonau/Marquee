<script lang="ts">
  import { app } from '../state.svelte';
  import { ago, hours } from '../format';
  import type { GameHistory } from '../api/contracts';

  let { editionId = '', portId = '' }: { editionId?: string; portId?: string } = $props();
  let history = $state<GameHistory | null>(null);
  let failed = $state(false);

  $effect(() => {
    const edition = editionId;
    const port = portId;
    history = null;
    failed = false;
    const request = port
      ? app.api.portSessions(port)
      : app.api.gameSessions(edition);
    void request.then((next) => {
      if (edition === editionId && port === portId) history = next;
    }).catch(() => {
      if (edition === editionId && port === portId) failed = true;
    });
  });
</script>

<section class="history">
  <h3>History</h3>
  {#if failed}
    <p>Play history is unavailable right now.</p>
  {:else if !history}
    <p>Loading play history…</p>
  {:else if history.sessions.length === 0}
    <p>No recorded sessions yet.</p>
  {:else}
    <p>{history.week_seconds > 0 ? `${hours(history.week_seconds / 60)} this week. ` : ''}Recent sessions:</p>
    <ul>
      {#each history.sessions.slice(0, 8) as session (session.id)}
        <li>
          <span>{ago(session.started_at)}</span>
          <span>{session.state === 'failed' ? 'Did not stay open' : session.state === 'running' ? 'Playing now' : session.duration_seconds ? hours(session.duration_seconds / 60) : 'Ended'}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h3 { margin: 0; font-size: 18px; }
  p, li { margin: 0; color: var(--text-soft); font-size: 14px; line-height: 1.5; }
  ul { list-style: none; padding: 0; display: grid; gap: 6px; }
  li { display: flex; justify-content: space-between; gap: 16px; }
  .history { display: grid; gap: 8px; margin-top: 8px; }
</style>
