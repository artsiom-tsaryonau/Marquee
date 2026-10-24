<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  let { label, children, icon = 'more', text, closeOnAction = true }: { label: string; children: Snippet; icon?: IconName; text?: string; closeOnAction?: boolean } = $props();
  let root: HTMLDetailsElement;
  onMount(() => {
    function outside(event: PointerEvent) { if (event.target instanceof Node && !root.contains(event.target)) root.open = false; }
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape' && root.open) { event.preventDefault(); event.stopPropagation(); root.open = false; root.querySelector('summary')?.focus(); }
    }
    function action(event: MouseEvent) { if (closeOnAction && event.target instanceof Element && event.target.closest('button, a')) { root.open = false; root.querySelector('summary')?.focus(); } }
    function leave(event: FocusEvent) { if (!(event.relatedTarget instanceof Node) || !root.contains(event.relatedTarget)) root.open = false; }
    document.addEventListener('pointerdown', outside);
    root.addEventListener('keydown', key);
    root.addEventListener('click', action);
    root.addEventListener('focusout', leave);
    return () => { document.removeEventListener('pointerdown', outside); root.removeEventListener('keydown', key); root.removeEventListener('click', action); root.removeEventListener('focusout', leave); };
  });
</script>

<details bind:this={root} class="music-menu">
  <summary class:with-label={!!text} data-nav aria-label={label} title={label}><Icon name={icon} size={19} />{#if text}<span class="visible-label">{text}</span>{/if}</summary>
  <div class="menu-content">{@render children()}</div>
</details>

<style>
  .music-menu { position: relative; flex: 0 0 auto; }
  summary { display: grid; place-items: center; width: var(--music-control-size); height: var(--music-control-size); border-radius: 50%; color: var(--text-dim); cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  summary.with-label { border-radius:6px; display: flex; gap: 7px; width: auto; padding: 0 10px; font-size: 12px; white-space: nowrap; }
  @media(max-width:900px) { :global(.music-player) summary.with-label { width: var(--music-control-size); padding: 0; justify-content: center; } :global(.music-player) .visible-label { display: none; } }
  summary:hover, .music-menu[open] summary { color: var(--text); background: var(--field); }
  summary:focus-visible { outline: 2px solid var(--music-text); outline-offset: 2px; }
  .menu-content { position: absolute; z-index: 45; right: 0; top: calc(100% + 6px); min-width: 210px; max-width: min(320px, 75vw); padding: 6px; border-radius: 12px; background: var(--surface-strong); border: 1px solid var(--line); box-shadow: 0 8px 30px #0004; }
  .menu-content :global(button), .menu-content :global(a), .menu-content :global(label) { width: 100%; box-sizing: border-box; display: flex; align-items: center; gap: 9px; min-height: 40px; padding: 10px 12px; border: 0; border-radius: 7px; background: transparent; color: var(--text); text-align: left; font: inherit; font-size: 13px; text-decoration: none; }
  .menu-content :global(button:hover), .menu-content :global(a:hover) { background: var(--field); }
  .menu-content :global(button) { cursor: pointer; } .menu-content :global(button:disabled) { opacity: .4; cursor: default; }
  .menu-content :global(.danger) { color: var(--bad); }
  :global(.music-player) .menu-content { top: auto; bottom: calc(100% + 10px); }
  @media(pointer:coarse) { summary { width: 44px; height: 44px; } }
</style>
