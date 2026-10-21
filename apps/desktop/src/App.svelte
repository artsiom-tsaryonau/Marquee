<script lang="ts">
  import { fade, fly, scale } from 'svelte/transition';
  import { onMount } from 'svelte';
  import { emulatorName } from './lib/presentation';
  import { uiIsStale } from './lib/build';
  import { app, type Route } from './lib/state.svelte';
  import { startFocusEngine } from './lib/focus';
  import Backdrop from './lib/components/Backdrop.svelte';
  import Icon, { type IconName } from './lib/components/Icon.svelte';
  import Logo from './lib/components/Logo.svelte';
  import Artwork from './lib/components/Artwork.svelte';
  import InstallDialog from './lib/components/InstallDialog.svelte';
  import Home from './lib/screens/Home.svelte';
  import Library from './lib/screens/Library.svelte';
  import GamePage from './lib/screens/GamePage.svelte';
  import Ports from './lib/screens/Ports.svelte';
  import Tools from './lib/screens/Tools.svelte';
  import Settings from './lib/screens/Settings.svelte';

  const nav: { route: Route; label: string; icon: IconName }[] = [
    { route: 'home', label: 'Home', icon: 'home' },
    { route: 'library', label: 'Library', icon: 'grid' },
    { route: 'tools', label: 'Tools', icon: 'controller' },
    { route: 'ports', label: 'Ports', icon: 'puzzle' },
    { route: 'settings', label: 'Settings', icon: 'settings' },
  ];

  let booting = $state(new URLSearchParams(location.search).get('splash') !== '0');
  let now = $state(new Date());
  let stale = $state(false);
  const checkStale = () => void uiIsStale().then((v) => (stale = v));

  const dur = $derived(app.motionReduced ? 0 : 260);

  // Decorative animation stops when nobody is looking at it: tab hidden, window
  // unfocused, or a game is running on top. Keeps the GPU free for the emulator.
  let away = $state(typeof document !== 'undefined' && (document.hidden || !document.hasFocus()));
  $effect(() => {
    const update = () => (away = document.hidden || !document.hasFocus());
    document.addEventListener('visibilitychange', update);
    window.addEventListener('focus', update);
    window.addEventListener('blur', update);
    return () => {
      document.removeEventListener('visibilitychange', update);
      window.removeEventListener('focus', update);
      window.removeEventListener('blur', update);
    };
  });

  $effect(() => {
    const root = document.documentElement;
    root.dataset.theme = app.theme;
    root.dataset.mode = app.mode;
    root.dataset.motion = app.motionReduced ? 'reduce' : 'full';
    root.dataset.effects = app.visualEffects;
    root.dataset.idle = away || app.running ? 'yes' : 'no';
  });

  onMount(() => {
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    const updateMotion = () => (app.systemReduceMotion = motion.matches);
    updateMotion();
    motion.addEventListener('change', updateMotion);
    void app.boot();
    checkStale();
    window.addEventListener('focus', checkStale);
    // Dev shortcuts: ?route=library&mode=big&theme=light&splash=0&empty=1
    const q = new URLSearchParams(location.search);
    const r = q.get('route') as Route | null;
    if (r) app.route = r;
    if (q.get('mode') === 'big' || q.get('mode') === 'desktop') app.mode = q.get('mode') as 'big' | 'desktop';
    if (q.get('theme') === 'light' || q.get('theme') === 'dark') app.theme = q.get('theme') as 'light' | 'dark';
    if (q.get('motion') === '0') app.reduceMotion = true;
    const stop = startFocusEngine({
      back: () => {
        if (app.installToolId) app.closeInstall();
        else app.back();
      },
      onInput: (l) => (app.lastInput = l),
    });
    const refreshSources = () => {
      if (!document.hidden && document.hasFocus() && app.lutrisSource?.config.enabled && !app.lutrisLoading) void app.loadLutris();
    };
    window.addEventListener('focus', refreshSources);
    const sourcePoll = setInterval(refreshSources, 60000);
    const clock = setInterval(() => { if (!document.hidden && document.hasFocus()) now = new Date(); }, 15000);
    if (booting) setTimeout(() => (booting = false), app.motionReduced ? 0 : 1500);
    return () => {
      stop();
      app.disconnect();
      motion.removeEventListener('change', updateMotion);
      clearInterval(clock);
      clearInterval(sourcePoll);
      window.removeEventListener('focus', refreshSources);
      window.removeEventListener('focus', checkStale);
    };
  });

  const time = $derived(now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }));
</script>

<Backdrop blur={app.route !== 'game'} />

<div class="shell" inert={!!(app.launching || app.running || app.installToolId)}>
  {#if app.mode === 'desktop'}
    <nav class="side glass-panel" class:expanded={app.sidebarExpanded} aria-label="Main">
      <div class="brand"><span class="mark">M</span><span class="word">Marquee</span></div>
      {#each nav as n (n.route)}
        <button class="nav" class:on={app.route === n.route} aria-current={app.route === n.route ? 'page' : undefined} data-nav onclick={() => app.go(n.route)}>
          <Icon name={n.icon} size={22} />
          <span>{n.label}</span>
        </button>
      {/each}
      <div class="spacer"></div>
      <button class="nav add" data-nav onclick={() => app.addGames()}>
        <Icon name="plus" size={22} /><span>Add games</span>
      </button>
      <button class="nav" data-nav onclick={() => (app.mode = 'big')}>
        <Icon name="tv" size={22} /><span>Big Screen</span>
      </button>
    </nav>
  {:else}
    <nav class="top" aria-label="Main">
      <div class="brand"><span class="mark">M</span></div>
      <div class="tabs" data-nav-row>
        {#each nav as n (n.route)}
          <button class="tab" class:on={app.route === n.route} aria-current={app.route === n.route ? 'page' : undefined} data-nav onclick={() => app.go(n.route)}>{n.label}</button>
        {/each}
      </div>
      <div class="status">
        <button class="tab" data-nav onclick={() => app.addGames()} aria-label="Add games"><Icon name="plus" size={20} /></button>
        <button class="tab" data-nav onclick={() => (app.mode = 'desktop')} aria-label="Desktop mode"><Icon name="monitor" size={20} /></button>
        <span class="time">{time}</span>
      </div>
    </nav>
  {/if}

  <main>
    {#key app.route + (app.route === 'game' ? app.gameId : '')}
      <!-- Keep navigation opaque and replace the page immediately. A delayed
           whole-page fade exposes the backdrop and composites two large pages
           at once; on the desktop this looks like a window flash. -->
      <div class="screen">
        {#if app.route === 'home'}<Home />
        {:else if app.route === 'library'}<Library />
        {:else if app.route === 'ps2'}<Library collection="ps2" />
        {:else if app.route === 'game'}<GamePage />
        {:else if app.route === 'tools'}<Tools />
        {:else if app.route === 'ports'}<Ports />
        {:else}<Settings />{/if}
      </div>
    {/key}
  </main>

  {#if app.mode === 'big'}
    <footer class="hints">
      <span><kbd class="a">A</kbd> Select</span>
      <span><kbd class="b">B</kbd> Back</span>
    </footer>
  {/if}
</div>

{#if app.installToolId}
  <InstallDialog />
{/if}

{#if app.launching || app.running}
  {@const g = (app.launching ?? app.running)!}
  <div class="launch" style="--accent-h:{g.art.hue2}" transition:fade={{ duration: dur }}>
    <div class="launch-art"><Artwork art={g.art} /></div>
    <div class="launch-body" in:scale={{ start: 0.94, duration: dur * 1.5 }}>
      <Logo game={g} size="xl" />
      {#if app.launching}
        <div class="loader"><span></span></div>
        <p>Starting via {g.source === 'steam' ? 'Steam' : g.source === 'port' ? 'native port' : g.source === 'rom' ? emulatorName(g) : 'Lutris'}…</p>
      {:else}
        <p class="running"><span class="live"></span> Playing now</p>
        <button class="btn glass" data-nav onclick={() => (app.dismissOverlay())}>Return to Marquee <kbd>Esc</kbd></button>
      {/if}
    </div>
  </div>
{/if}

{#if stale}
  <button class="stale glass-panel" onclick={() => location.reload()}>
    Marquee was rebuilt. Reload to match it
  </button>
{/if}

{#if app.toast}
  <div class="toast glass-panel" role="status" transition:fly={{ y: 20, duration: dur }}>{app.toast}</div>
{/if}

{#if booting}
  <div class="boot" out:fade={{ duration: 500 }}>
    <div class="boot-mark">
      {#each 'MARQUEE'.split('') as c, i (i)}<span style="--i:{i}">{c}</span>{/each}
    </div>
  </div>
{/if}

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }
  :global([data-mode='big']) .shell {
    flex-direction: column;
  }
  main {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .screen {
    grid-area: 1 / 1;
    min-width: 0;
  }

  /* Desktop sidebar: icons, labels slide out on hover. */
  .side {
    position: sticky;
    top: 12px;
    height: calc(100vh - 24px);
    margin: 12px 0 12px 12px;
    width: 64px;
    padding: 14px 8px;
    border-radius: 22px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    z-index: 20;
    overflow: hidden;
    transition: width var(--side-close-duration) var(--ease);
  }
  .side.expanded,
  .side:hover,
  .side:focus-within {
    width: 220px;
    transition:
      width var(--side-open-duration) var(--ease) var(--side-open-delay);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 6px 14px;
    white-space: nowrap;
  }
  .mark {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    font-weight: 800;
    color: #e6e4f2;
    /* Quiet slate: the mark should not compete with the games. */
    background: linear-gradient(150deg, #4b4a68, #2a2940);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.12), 0 4px 12px -4px rgb(0 0 0 / 0.6);
  }
  .word {
    font-weight: 800;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    font-size: 14px;
  }
  .nav {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 46px;
    padding: 0 13px;
    border: 0;
    border-radius: 14px;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    outline: none;
    transition: background 160ms, color 160ms;
  }
  .nav :global(svg) {
    flex: none;
  }
  .nav span,
  .word {
    opacity: 0;
    transition: opacity 160ms var(--ease);
  }
  .side.expanded :is(.nav span, .word),
  .side:hover :is(.nav span, .word),
  .side:focus-within :is(.nav span, .word) {
    opacity: 1;
    transition: opacity 280ms var(--ease) var(--side-label-delay);
  }
  .nav:hover,
  .nav:focus-visible {
    background: var(--field);
    color: var(--text);
  }
  .nav.on {
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .nav.on::before {
    content: '';
    position: absolute;
    left: -8px;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 3px;
    background: var(--accent);
    box-shadow: 0 0 12px var(--accent);
  }
  .nav.add {
    color: var(--accent);
  }
  .spacer {
    flex: 1;
  }

  /* Big Screen top bar. */
  .top {
    position: sticky;
    top: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px var(--gutter);
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg) 85%, transparent), transparent);
  }
  .tabs {
    display: flex;
    gap: 6px;
    padding: 5px;
    border-radius: 999px;
    background: var(--field);
    backdrop-filter: var(--overlay-filter);
    -webkit-backdrop-filter: var(--overlay-filter);
  }
  .tab {
    border: 0;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-size: 18px;
    font-weight: 700;
    padding: 10px 22px;
    border-radius: 999px;
    cursor: pointer;
    outline: none;
    display: inline-flex;
    align-items: center;
    transition: background 160ms, color 160ms, transform 160ms;
  }
  .tab:focus-visible {
    transform: scale(1.06);
    box-shadow: 0 0 0 2px var(--accent), 0 0 24px var(--accent);
  }
  .tab.on {
    background: var(--text);
    color: var(--bg);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .time {
    font-size: 20px;
    font-weight: 700;
    margin-left: 10px;
    font-variant-numeric: tabular-nums;
  }
  .hints {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: flex-end;
    gap: 26px;
    padding: 16px var(--gutter);
    font-weight: 600;
    background: linear-gradient(0deg, color-mix(in srgb, var(--bg) 90%, transparent), transparent);
    pointer-events: none;
  }
  .hints span {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    border-radius: 50%;
    font: inherit;
    font-size: 12px;
    font-weight: 800;
    background: var(--field-strong);
  }
  kbd.a {
    background: #22c55e;
    color: #04210f;
  }
  kbd.b {
    background: #ef4444;
    color: #fff;
  }

  /* Launch / running overlay. */
  .launch {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: #000;
  }
  .launch-art {
    position: absolute;
    inset: 0;
    opacity: 0.55;
    animation: drift 14s ease-in-out infinite alternate;
  }
  @keyframes drift {
    from {
      transform: scale(1.05);
    }
    to {
      transform: scale(1.18) translate(-2%, -1%);
    }
  }
  .launch::after {
    content: '';
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at center, transparent 20%, rgb(0 0 0 / 0.8));
  }
  .launch-body {
    position: relative;
    z-index: 1;
    display: grid;
    justify-items: center;
    gap: 22px;
    text-align: center;
    color: #fff;
    padding: 24px;
  }
  .launch-body p {
    margin: 0;
    color: rgb(255 255 255 / 0.75);
    font-weight: 600;
  }
  .loader {
    width: 220px;
    height: 4px;
    border-radius: 4px;
    background: rgb(255 255 255 / 0.15);
    overflow: hidden;
  }
  .loader span {
    display: block;
    height: 100%;
    width: 40%;
    border-radius: inherit;
    background: hsl(var(--accent-h) 95% 70%);
    box-shadow: 0 0 16px hsl(var(--accent-h) 95% 60%);
    animation: load 1.1s ease-in-out infinite;
  }
  @keyframes load {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }
  .running {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .live {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #22c55e;
    box-shadow: 0 0 0 0 rgb(34 197 94 / 0.7);
    animation: live 1.6s infinite;
  }
  @keyframes live {
    70% {
      box-shadow: 0 0 0 12px rgb(34 197 94 / 0);
    }
  }
  .launch-body kbd {
    border-radius: 6px;
    min-width: 0;
    padding: 0 6px;
    height: 20px;
    margin-left: 6px;
    background: rgb(255 255 255 / 0.18);
  }

  .stale {
    position: fixed;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 70;
    padding: 10px 18px;
    border-radius: 14px;
    font-weight: 700;
    background: var(--accent);
    color: #fff;
  }

  .toast {
    position: fixed;
    bottom: 28px;
    left: 50%;
    transform: translate(-50%, 0);
    z-index: 60;
    padding: 12px 20px;
    border-radius: 14px;
    font-weight: 600;
    background: var(--surface-strong);
  }

  /* Boot splash. */
  .boot {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: radial-gradient(circle at 50% 60%, #1e1036, #05040a 70%);
  }
  .boot-mark {
    display: flex;
    gap: 0.06em;
    font-size: clamp(40px, 8vw, 96px);
    font-weight: 800;
    letter-spacing: 0.1em;
  }
  .boot-mark span {
    background: linear-gradient(180deg, #fff, #c4b5fd 60%, #f472b6);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    opacity: 0;
    filter: blur(12px);
    animation: letter 700ms var(--ease) forwards;
    animation-delay: calc(var(--i) * 70ms);
  }
  @keyframes letter {
    to {
      opacity: 1;
      filter: blur(0);
    }
  }

  :global(:root[data-motion='reduce']) .side,
  :global(:root[data-motion='reduce']) .side:hover,
  :global(:root[data-motion='reduce']) .side:focus-within {
    transition-duration: 0.001ms !important;
    transition-delay: 0ms !important;
  }
  :global(:root[data-motion='reduce']) .nav span,
  :global(:root[data-motion='reduce']) .word,
  :global(:root[data-motion='reduce']) .side:hover :is(.nav span, .word),
  :global(:root[data-motion='reduce']) .side:focus-within :is(.nav span, .word) {
    transition-duration: 0.001ms !important;
    transition-delay: 0ms !important;
  }

  @media (max-width: 640px) {
    .side,
    .side:hover,
    .side:focus-within {
      position: fixed;
      top: auto;
      left: 8px;
      right: 8px;
      bottom: 8px;
      width: auto;
      height: 60px;
      margin: 0;
      padding: 6px;
      flex-direction: row;
      justify-content: space-around;
      transition: none;
    }
    .side :is(.brand, .spacer, .add, .nav span) {
      display: none;
    }
    .nav.on::before {
      display: none;
    }
    main {
      padding-bottom: 76px;
    }
    .tab {
      font-size: 14px;
      padding: 8px 12px;
    }
  }
</style>
