<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import { installables } from '../install';
  import Icon from './Icon.svelte';

  let {
    tone = 'glass',
    size = 'default',
  }: { tone?: 'glass' | 'primary'; size?: 'default' | 'small' | 'big' } = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined;
  let menuEl = $state<HTMLDivElement | undefined>(undefined);
  let menuPos = $state({ top: 0, left: 0 });

  function place() {
    const btn = root?.querySelector('button');
    if (!btn) return;
    const r = btn.getBoundingClientRect();
    const width = 280;
    const left = Math.min(r.left, window.innerWidth - width - 12);
    menuPos = { top: r.bottom + 8, left: Math.max(12, left) };
  }

  function toggle() {
    if (!open) place();
    open = !open;
  }

  function pick(toolId: string, optionId: string) {
    open = false;
    app.openInstall(toolId, optionId);
  }

  function onDoc(e: PointerEvent) {
    const t = e.target as Node;
    if (open && root && !root.contains(t) && !menuEl?.contains(t)) open = false;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    open = false;
  }

  $effect(() => {
    if (!open || !menuEl) return;
    document.body.appendChild(menuEl);
  });

  onMount(() => {
    document.addEventListener('pointerdown', onDoc);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('resize', place);
    return () => {
      document.removeEventListener('pointerdown', onDoc);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('resize', place);
    };
  });

  const label = $derived(app.installingPcsx2 ? 'Installing…' : 'Install');
</script>

<div class="wrap" bind:this={root}>
  <button
    class="btn {tone} {size === 'default' ? '' : size}"
    data-nav
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={toggle}
  >
    <Icon name="download" />
    {label}
    <Icon name="chevron" size={16} />
  </button>
</div>

{#if open}
  <div
    bind:this={menuEl}
    class="menu glass-panel"
    role="menu"
    data-nav-row
    style="top:{menuPos.top}px;left:{menuPos.left}px"
  >
    {#each installables as tool (tool.id)}
      <p class="group">{tool.title}</p>
      <p class="sub">{tool.subtitle}</p>
      {#each tool.options as option (option.id)}
        <button class="item" role="menuitem" data-nav onclick={() => pick(tool.id, option.id)}>
          <span class="titles">
            <strong>{option.title}</strong>
            <span>{option.hint}</span>
          </span>
          <Icon name="play" size={14} />
        </button>
      {/each}
    {/each}
  </div>
{/if}

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }
  .wrap :global(.btn) {
    flex: none;
  }
  .menu {
    position: fixed;
    z-index: 36;
    min-width: 280px;
    padding: 8px;
    border-radius: 16px;
    display: grid;
    gap: 2px;
  }
  .group {
    margin: 6px 10px 0;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .sub {
    margin: 0 10px 6px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    text-align: left;
    border: 0;
    border-radius: 12px;
    padding: 10px 12px;
    background: none;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--field);
  }
  .titles {
    display: grid;
    gap: 1px;
  }
  .titles span {
    font-size: 12px;
    color: var(--text-dim);
  }
  .item :global(svg) {
    flex: none;
    color: var(--accent);
  }
</style>
