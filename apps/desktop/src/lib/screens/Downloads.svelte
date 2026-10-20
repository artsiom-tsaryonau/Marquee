<script lang="ts">
  import { app } from '../state.svelte';
  import Icon from '../components/Icon.svelte';

  const active = $derived(app.downloads.filter((d) => d.state !== 'done'));
  const done = $derived(app.downloads.filter((d) => d.state === 'done'));
</script>

<div class="downloads">
  <h1>Downloads</h1>
  {#if !active.length}
    <p class="empty">Nothing downloading. Ports, emulators and Proton updates show up here.</p>
  {/if}
  <ul>
    {#each active as d (d.id)}
      <li class="glass-panel">
        <div class="row">
          <strong>{d.name}</strong>
          <span class="state {d.state}">{d.state === 'downloading' ? d.speed : d.state}</span>
        </div>
        <div class="bar"><div class="fill" class:moving={d.state === 'downloading'} style="width:{d.progress * 100}%"></div></div>
        <div class="row small">
          <span>{Math.round(d.progress * 100)}%</span>
          <span class="acts">
            {#if d.state === 'downloading'}
              <button data-nav onclick={() => (d.state = 'paused')} aria-label="Pause"><Icon name="pause" size={16} /></button>
            {:else}
              <button data-nav onclick={() => (d.state = 'downloading')} aria-label="Resume"><Icon name="play" size={16} /></button>
            {/if}
            <button data-nav onclick={() => app.notify('Opened log')}>Log</button>
          </span>
        </div>
      </li>
    {/each}
  </ul>
  {#if done.length}
    <h2>Completed</h2>
    <ul>
      {#each done as d (d.id)}
        <li class="glass-panel done"><Icon name="check" size={18} /> {d.name}</li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .downloads {
    padding: 36px var(--gutter);
    max-width: 900px;
  }
  h1 {
    font-size: 32px;
    margin: 0 0 20px;
  }
  h2 {
    font-size: 16px;
    color: var(--text-dim);
    margin: 28px 0 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 12px;
  }
  li {
    padding: 16px 18px;
    border-radius: 16px;
    display: grid;
    gap: 10px;
  }
  li.done {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-soft);
  }
  li.done :global(svg) {
    color: var(--ok);
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .small {
    font-size: 13px;
    color: var(--text-dim);
  }
  .state {
    font-size: 13px;
    color: var(--text-dim);
    text-transform: capitalize;
  }
  .bar {
    height: 8px;
    border-radius: 999px;
    background: var(--field);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 50%, #fff));
    transition: width 400ms linear;
  }
  .fill.moving {
    background-size: 40px 100%;
    background-image:
      linear-gradient(135deg, rgb(255 255 255 / 0.2) 25%, transparent 25%, transparent 50%, rgb(255 255 255 / 0.2) 50%, rgb(255 255 255 / 0.2) 75%, transparent 75%),
      linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 50%, #fff));
    animation: stripes 0.8s linear infinite;
  }
  @keyframes stripes {
    to {
      background-position: 40px 0, 0 0;
    }
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .acts button {
    display: inline-flex;
    align-items: center;
    background: var(--field);
    border: 1px solid var(--line);
    color: var(--text);
    border-radius: 8px;
    padding: 4px 10px;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .empty {
    color: var(--text-dim);
  }
</style>
