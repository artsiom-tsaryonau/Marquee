<script lang="ts">
  // Empty shelf slot that keeps Home/Library looking like a console wall of covers.
  let {
    shape = 'cover',
    label = 'Add PS2 folder',
    onclick,
  }: {
    shape?: 'cover' | 'wide';
    label?: string;
    onclick?: () => void;
  } = $props();
</script>

<div class="wrap {shape}">
  {#if onclick}
    <button class="card" data-nav {onclick} aria-label={label}>
      <span class="plus" aria-hidden="true">+</span>
    </button>
  {:else}
    <div class="card inert" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .wrap {
    flex: none;
    width: var(--cover-w);
  }
  .wrap.wide {
    width: calc(var(--cover-w) * 2.1);
  }
  .card {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 2 / 3;
    border: 1px dashed rgb(255 255 255 / 0.22);
    border-radius: var(--radius);
    background: rgb(255 255 255 / 0.07);
    color: rgb(255 255 255 / 0.55);
    box-shadow: 0 10px 30px -12px rgb(0 0 0 / 0.5);
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .card.inert {
    cursor: default;
  }
  .plus {
    font-size: 28px;
    font-weight: 300;
    line-height: 1;
  }
</style>
