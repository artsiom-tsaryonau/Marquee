<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { app } from '../state.svelte';
  import { installable } from '../install';
  import Icon from './Icon.svelte';

  const tool = $derived(app.installToolId ? installable(app.installToolId) : undefined);
  const dur = $derived(app.reduceMotion ? 0 : 240);
  const busy = $derived(app.installingPcsx2);

  function run(optionId: string) {
    if (!tool || busy) return;
    void app.runInstall(tool.id, optionId);
  }
</script>

{#if tool}
  <div class="scrim" transition:fade={{ duration: dur }} onclick={() => app.closeInstall()} role="presentation"></div>
  <div
    class="dialog glass-panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="install-title"
    transition:fly={{ y: 24, duration: dur }}
  >
    <header>
      <div>
        <h2 id="install-title">Install {tool.title}</h2>
        <p>{tool.subtitle}. Marquee will not download BIOS files or commercial games. Click an option to run it.</p>
      </div>
      <button class="close" data-nav onclick={() => app.closeInstall()} aria-label="Close"><Icon name="x" /></button>
    </header>

    {#if app.installProgress}
      <p class="progress" role="status">{app.installProgress}</p>
    {/if}

    <div class="options">
      {#each tool.options as option (option.id)}
        <button
          class="option"
          class:on={app.installOptionId === option.id}
          data-nav
          disabled={busy && option.run.type !== 'setup'}
          onclick={() => run(option.id)}
        >
          <span class="mark">
            {#if busy && app.installOptionId === option.id && option.run.type !== 'setup'}
              <span class="spin"></span>
            {:else}
              <Icon name={option.run.type === 'setup' ? 'settings' : 'download'} size={18} />
            {/if}
          </span>
          <span class="copy">
            <strong>{option.title}</strong>
            <span>{option.detail}</span>
          </span>
          <span class="go">{busy && app.installOptionId === option.id ? 'Working' : 'Run'}</span>
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
  }
  .dialog {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow: auto;
    border-radius: 24px;
    padding: 24px;
    background: var(--surface-strong);
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 16px;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 22px;
  }
  header p {
    margin: 0;
    color: var(--text-dim);
    font-size: 14px;
  }
  .close {
    flex: none;
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--line);
    background: var(--field);
    color: var(--text);
    cursor: pointer;
  }
  .progress {
    margin: 0 0 14px;
    padding: 10px 12px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 16%, var(--field));
    font-size: 13px;
    font-weight: 600;
  }
  .options {
    display: grid;
    gap: 8px;
  }
  .option {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 14px;
    width: 100%;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: 16px;
    padding: 14px 16px;
    background: var(--field);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    outline: none;
    transition: transform 180ms var(--ease), box-shadow 180ms, border-color 180ms;
  }
  .option:hover:not(:disabled),
  .option:focus-visible {
    transform: translateY(-2px);
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .option.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, var(--field));
  }
  .option:disabled {
    cursor: progress;
    opacity: 0.7;
  }
  .mark {
    width: 36px;
    height: 36px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--field-strong);
    color: var(--accent);
  }
  .copy {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .copy span {
    color: var(--text-dim);
    font-size: 13px;
  }
  .go {
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
</style>
