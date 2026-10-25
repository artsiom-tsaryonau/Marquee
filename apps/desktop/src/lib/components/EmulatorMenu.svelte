<script lang="ts">
  import { tick } from 'svelte';
  import type { Runner } from '../api/contracts';
  import { emulatorChoices } from '../runnerChoices';
  import Icon from './Icon.svelte';
  let { runners, defaultId, value, disabled = false, onchange }: {
    runners: Runner[]; defaultId?: string; value?: string; disabled?: boolean;
    onchange: (value: string | null) => Promise<void>;
  } = $props();
  const choices = $derived(emulatorChoices(runners, defaultId));
  const selected = $derived(value === defaultId ? '' : value ?? '');
  const label = $derived(choices.find(r => r.value === selected)?.label ?? (value ? 'Selected emulator unavailable' : 'No emulator available'));
  let open = $state(false);
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let menu = $state<HTMLDivElement>();
  async function show(last = false) {
    if (disabled) return;
    open = true; await tick();
    const items = menu?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)');
    if (items?.length) (last ? items[items.length - 1] : [...items].find(b => b.getAttribute('aria-checked') === 'true') ?? items[0]).focus();
  }
  function close(focus = false) { open = false; if (focus) trigger?.focus(); }
  async function pick(next: string) { close(true); await onchange(next || null); }
  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true); }
    else if (event.key === 'Tab') close();
    else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const items = [...menu!.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
      if (!items.length) return;
      const i = items.indexOf(document.activeElement as HTMLButtonElement);
      items[event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (i + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length].focus();
    }
  }
  $effect(() => { if (disabled) open = false; });
</script>

<svelte:window onpointerdown={e => { if (open && !root?.contains(e.target as Node)) close(); }} />
<div class="emulator-menu" bind:this={root} onfocusout={e => { if (!root?.contains(e.relatedTarget as Node)) close(); }}>
  <button class="btn glass trigger" data-nav bind:this={trigger} {disabled} aria-label="Emulator" aria-haspopup="menu" aria-expanded={open} title="Saved for this game. Each emulator keeps its own saves and states." onclick={() => open ? close() : show()} onkeydown={e => { if (['ArrowDown','ArrowUp'].includes(e.key)) { e.preventDefault(); void show(e.key === 'ArrowUp'); } }}>
    <Icon name="controller" size={17} /><span>{label}</span><Icon name="chevron" size={15} />
  </button>
  {#if open}
    <div class="options glass-panel" role="menu" aria-label="Choose emulator" tabindex="-1" bind:this={menu} onkeydown={key}>
      {#each choices as choice (choice.id)}
        <button type="button" role="menuitemradio" aria-checked={choice.value === selected} disabled={!choice.available} onclick={() => pick(choice.value)}><span>{choice.label}</span>{#if choice.value === selected}<Icon name="check" size={16} />{/if}</button>
      {/each}
      {#if value && !choices.some(r => r.id === value)}<button role="menuitemradio" aria-checked="true" disabled>Selected emulator unavailable</button>{/if}
      {#if !choices.length}<p>No compatible emulator installed.</p>{/if}
    </div>
  {/if}
</div>

<style>
  .emulator-menu { position: relative; min-width: 0; max-width: 100%; }
  .trigger { height: var(--button-height,46px); gap: 10px; width: 320px; max-width: 100%; justify-content: flex-start; }
  .trigger span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; text-align: left; }
  .options { position: absolute; top: calc(100% + 6px); left: 0; z-index: 40; min-width: 100%; width: max-content; max-width: min(380px,calc(100vw - 48px)); max-height: 300px; overflow-y: auto; padding: 6px; border-radius: 8px; box-shadow: 0 12px 32px #0008; }
  .options button { display: flex; align-items: center; justify-content: space-between; gap: 16px; width: 100%; min-height: 40px; padding: 10px 12px; border: 0; border-radius: 5px; background: transparent; color: var(--text-soft); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
  .options button:hover, .options button:focus-visible { background: var(--field); color: var(--text); outline: 2px solid var(--accent); outline-offset: -2px; }
  .options button[aria-checked="true"] { color: var(--text); background: var(--field); }
  .options button:disabled { opacity: .5; cursor: default; }
  .options p { font-size: 12px; color: var(--text-dim); padding: 8px; }
  @media(max-width:600px) { .emulator-menu { flex-basis: 100%; } .trigger { width: 100%; } }
</style>
