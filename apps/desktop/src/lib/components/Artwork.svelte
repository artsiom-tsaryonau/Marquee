<script lang="ts">
  // Procedural stand-in for cover and hero images: one 1000×1000 scene,
  // cropped to whatever box it fills (2:3 cover, wide hero).
  import type { Art } from '../types';

  let { art, grain = true }: { art: Art; grain?: boolean } = $props();
  const uid = $props.id();

  const h = $derived(art.hue);
  const h2 = $derived(art.hue2);

  function rng(seed: number) {
    return () => {
      seed = (seed * 1664525 + 1013904223) % 4294967296;
      return seed / 4294967296;
    };
  }

  const stars = $derived.by(() => {
    const r = rng(h * 31 + h2);
    return Array.from({ length: 60 }, () => ({ x: r() * 1000, y: r() * 560, s: r() * 2.2 + 0.4, o: r() * 0.7 + 0.2 }));
  });

  const shards = $derived.by(() => {
    const r = rng(h * 97 + 5);
    return Array.from({ length: 14 }, (_, i) => {
      const cx = r() * 1000, cy = r() * 1000, s = 120 + r() * 320;
      const a = r() * Math.PI * 2;
      const pts = [0, 2.1, 4.2].map((o) => `${cx + Math.cos(a + o + r() * 0.6) * s},${cy + Math.sin(a + o + r() * 0.6) * s}`);
      return { pts: pts.join(' '), light: 30 + i * 3, o: 0.25 + r() * 0.5 };
    });
  });

  function ridge(base: number, amp: number, seed: number) {
    const r = rng(seed);
    let d = `M0 1000 L0 ${base}`;
    for (let x = 0; x <= 1000; x += 50) d += ` L${x} ${base - r() * amp}`;
    return d + ' L1000 1000 Z';
  }

  function wave(y: number, amp: number, phase: number) {
    let d = `M0 1000 L0 ${y}`;
    for (let x = 0; x <= 1000; x += 20) d += ` L${x} ${y + Math.sin(x / 90 + phase) * amp}`;
    return d + ' L1000 1000 Z';
  }
</script>

<svg viewBox="0 0 1000 1000" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
  <defs>
    <linearGradient id="sky-{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="hsl({h} 70% 9%)" />
      <stop offset="0.55" stop-color="hsl({h} 60% 22%)" />
      <stop offset="1" stop-color="hsl({h2} 70% 38%)" />
    </linearGradient>
    <linearGradient id="hot-{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="hsl({h2} 95% 72%)" />
      <stop offset="1" stop-color="hsl({(h2 + 40) % 360} 90% 52%)" />
    </linearGradient>
    <radialGradient id="glow-{uid}" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="hsl({h2} 100% 70%)" stop-opacity="0.9" />
      <stop offset="1" stop-color="hsl({h2} 100% 60%)" stop-opacity="0" />
    </radialGradient>
    <linearGradient id="fade-{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0.5" stop-color="#000" stop-opacity="0" />
      <stop offset="1" stop-color="#000" stop-opacity="0.55" />
    </linearGradient>
    <mask id="sunmask-{uid}">
      <rect width="1000" height="1000" fill="#fff" />
      {#each [0, 1, 2, 3, 4, 5] as i (i)}
        <rect x="0" y={470 + i * 26} width="1000" height={4 + i * 2.5} fill="#000" />
      {/each}
    </mask>
    {#if grain}
      <filter id="grain-{uid}">
        <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.08 0" />
      </filter>
    {/if}
  </defs>

  <rect width="1000" height="1000" fill="url(#sky-{uid})" />
  {#each stars as s, i (i)}
    <circle cx={s.x} cy={s.y} r={s.s} fill="#fff" opacity={s.o} />
  {/each}

  {#if art.motif === 'sun'}
    <circle cx="500" cy="470" r="330" fill="url(#glow-{uid})" />
    <circle cx="500" cy="470" r="210" fill="url(#hot-{uid})" mask="url(#sunmask-{uid})" />
    <path d={ridge(700, 120, h)} fill="hsl({h} 55% 14%)" />
    <path d={ridge(800, 80, h + 9)} fill="hsl({h} 60% 8%)" />
  {:else if art.motif === 'rings'}
    <circle cx="500" cy="520" r="420" fill="url(#glow-{uid})" opacity="0.6" />
    {#each [60, 120, 190, 270, 360, 460] as r, i (r)}
      <circle cx="500" cy="520" {r} fill="none" stroke="hsl({h2} 90% {75 - i * 7}%)" stroke-width={14 - i * 2} opacity={1 - i * 0.13} />
    {/each}
    <circle cx="500" cy="520" r="34" fill="url(#hot-{uid})" />
  {:else if art.motif === 'peaks'}
    <circle cx="690" cy="300" r="110" fill="url(#hot-{uid})" opacity="0.95" />
    <circle cx="690" cy="300" r="260" fill="url(#glow-{uid})" opacity="0.5" />
    {#each [0, 1, 2, 3] as i (i)}
      <path d={ridge(520 + i * 110, 260 - i * 40, h * 3 + i)} fill="hsl({h} {50 - i * 6}% {30 - i * 6}%)" />
    {/each}
  {:else if art.motif === 'grid'}
    <rect y="560" width="1000" height="440" fill="hsl({h} 70% 6%)" />
    <ellipse cx="500" cy="560" rx="560" ry="90" fill="url(#glow-{uid})" />
    {#each Array.from({ length: 21 }, (_, i) => i) as i (i)}
      <line x1={500 + (i - 10) * 18} y1="560" x2={500 + (i - 10) * 160} y2="1000" stroke="hsl({h2} 95% 65%)" stroke-width="2" opacity="0.7" />
    {/each}
    {#each [0, 1, 2, 3, 4, 5, 6, 7] as i (i)}
      <line x1="0" x2="1000" y1={560 + Math.pow(i / 7, 2) * 440} y2={560 + Math.pow(i / 7, 2) * 440} stroke="hsl({h2} 95% 65%)" stroke-width="2" opacity="0.6" />
    {/each}
    <rect x="440" y="300" width="120" height="260" rx="6" fill="url(#hot-{uid})" opacity="0.9" />
  {:else if art.motif === 'waves'}
    <circle cx="300" cy="330" r="140" fill="url(#hot-{uid})" />
    <circle cx="300" cy="330" r="320" fill="url(#glow-{uid})" opacity="0.45" />
    {#each [0, 1, 2, 3, 4] as i (i)}
      <path d={wave(560 + i * 85, 34 - i * 4, i * 1.7 + h / 50)} fill="hsl({(h + i * 8) % 360} {65 - i * 5}% {34 - i * 6}%)" opacity="0.95" />
    {/each}
  {:else if art.motif === 'shards'}
    <circle cx="500" cy="500" r="480" fill="url(#glow-{uid})" opacity="0.35" />
    {#each shards as s, i (i)}
      <polygon points={s.pts} fill="hsl({i % 2 ? h2 : h} 80% {s.light}%)" opacity={s.o} style="mix-blend-mode: screen" />
    {/each}
  {:else if art.motif === 'orbit'}
    <circle cx="520" cy="470" r="380" fill="url(#glow-{uid})" opacity="0.4" />
    <circle cx="520" cy="470" r="200" fill="url(#hot-{uid})" />
    <circle cx="460" cy="420" r="200" fill="hsl({h} 60% 10%)" opacity="0.55" />
    <ellipse cx="520" cy="470" rx="380" ry="80" fill="none" stroke="hsl({h2} 90% 80%)" stroke-width="10" transform="rotate(-18 520 470)" opacity="0.85" />
    <circle cx="170" cy="760" r="34" fill="hsl({h2} 70% 70%)" />
  {:else}
    <rect y="760" width="1000" height="240" fill="hsl({h} 50% 7%)" />
    <ellipse cx="500" cy="760" rx="420" ry="60" fill="url(#glow-{uid})" />
    <rect x="430" y="200" width="140" height="560" fill="hsl({h} 40% 5%)" />
    <rect x="430" y="200" width="6" height="560" fill="url(#hot-{uid})" />
    <circle cx="500" cy="200" r="220" fill="url(#glow-{uid})" opacity="0.35" />
  {/if}

  <rect width="1000" height="1000" fill="url(#fade-{uid})" />
  {#if grain}
    <rect width="1000" height="1000" filter="url(#grain-{uid})" />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
