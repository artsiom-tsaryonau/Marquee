<script lang="ts">
  import { platformIconStyle } from '../platformIcons';
  let { platform, size = 24 }: { platform: string; size?: number } = $props();
  const icon = $derived(platformIconStyle(platform));
</script>

<!-- Original hardware-inspired miniatures; no trademark lettering or remote assets. -->
<svg class="platform-icon" width={size} height={size} viewBox="0 0 32 32" aria-hidden="true" fill="none">
  {#if icon.hardware === 'headset'}
    <path d="M16 17v9m0-3-9 7m9-7 9 7" stroke="#8794ac" stroke-width="2" /><rect x="2" y="5" width="28" height="14" rx="5" fill={icon.shell} /><path d="M6 9h8v6H6zm12 0h8v6h-8z" fill="#253149" /><path d="M12 6h8" stroke={icon.accent} />
  {:else if icon.hardware === 'handheld'}
    <rect x="7" y="2" width="18" height="28" rx="4" fill={icon.shell} /><rect x="10" y="5" width="12" height="12" rx="2" fill="#29354c" /><rect x="12" y="7" width="8" height="7" rx="1" fill={icon.screen || '#9fcad3'} />
    <path d="M11 21h6m-3-3v6" stroke="#263248" stroke-width="2" /><circle cx="21" cy="21" r="1.6" fill={icon.accent} /><circle cx="18" cy="24" r="1.6" fill={icon.accent} /><path d="m17 28 4-1" stroke="#dce8fa" stroke-opacity=".5" />
  {:else if icon.hardware === 'wide-handheld'}
    <rect x="1" y="7" width="30" height="19" rx="7" fill={icon.shell} /><rect x="9" y="9" width="14" height="14" rx="2" fill="#263248" /><rect x="11" y="11" width="10" height="9" rx="1" fill="#91bfd0" /><path d="M3 16h5m-2.5-2.5v5" stroke={icon.accent} stroke-width="1.5" /><circle cx="26" cy="14" r="1.3" fill={icon.accent} /><circle cx="28" cy="18" r="1.3" fill={icon.accent} />
  {:else if icon.hardware === 'dual-screen'}
    <rect x="6" y="1" width="20" height="14" rx="2" fill={icon.shell} /><rect x="9" y="3" width="14" height="10" rx="1" fill="#344158" /><rect x="10" y="4" width="12" height="7" fill={icon.accent} /><rect x="4" y="16" width="24" height="14" rx="3" fill={icon.shell} /><rect x="10" y="18" width="12" height="9" rx="1" fill="#8eafb8" /><path d="M5 22h4m-2-2v4" stroke="#344158" /><circle cx="25" cy="22" r="1.3" fill="#344158" />
  {:else if icon.hardware === 'nes'}
    <rect x="2" y="7" width="28" height="20" rx="3" fill={icon.shell} /><path d="M2 19h28v5H2z" fill="#8290a6" /><path d="M17 7h9v20h-9z" fill="#344158" /><path d="M6 12h9v5H6z" fill="#7c8a9f" /><rect x="6" y="22" width="4" height="2" fill={icon.accent} /><rect x="11" y="22" width="3" height="2" fill="#344158" />
  {:else if icon.hardware === 'snes'}
    <rect x="2" y="8" width="28" height="19" rx="6" fill={icon.shell} /><rect x="8" y="8" width="16" height="6" rx="2" fill="#7f849e" /><path d="M10 11h12" stroke="#344158" stroke-width="2" /><rect x="7" y="18" width="4" height="5" rx="1" fill={icon.accent} /><rect x="21" y="18" width="4" height="5" rx="1" fill={icon.accent} /><circle cx="14" cy="24" r="1" fill="#344158" /><circle cx="18" cy="24" r="1" fill="#344158" />
  {:else if icon.hardware === 'n64'}
    <path d="M6 8h20l5 12-3 7H4l-3-7z" fill={icon.shell} /><rect x="9" y="10" width="14" height="5" rx="2" fill="#202b40" /><path d="M11 12h10" stroke="#8290a6" /><circle cx="7" cy="22" r="2" fill={icon.accent} /><circle cx="13" cy="22" r="2" fill="#8cc8f3" /><circle cx="19" cy="22" r="2" fill="#db8192" /><circle cx="25" cy="22" r="2" fill="#88c6ad" />
  {:else if icon.hardware === 'genesis'}
    <rect x="1" y="7" width="30" height="21" rx="6" fill={icon.shell} /><ellipse cx="17" cy="15" rx="10" ry="7" fill="#202b40" /><path d="M10 13h13" stroke="#8b9bb8" stroke-width="2" /><path d="M8 20h18" stroke={icon.accent} stroke-width="2" /><rect x="5" y="23" width="7" height="2" rx="1" fill="#202b40" /><rect x="20" y="23" width="7" height="2" rx="1" fill="#202b40" />
  {:else if icon.hardware === 'ps1' || icon.hardware === 'dreamcast'}
    <rect x="2" y="5" width="28" height="24" rx="5" fill={icon.shell} /><circle cx="16" cy="15" r="8" fill="#8795ac" fill-opacity=".45" stroke="#59677e" /><circle cx="7" cy="8" r="2" fill={icon.accent} /><circle cx="25" cy="8" r="2" fill="#59677e" /><rect x="6" y="24" width="8" height="3" rx="1" fill="#344158" /><rect x="18" y="24" width="8" height="3" rx="1" fill="#344158" />
  {:else if icon.hardware === 'ps2' || icon.hardware === 'ps3' || icon.hardware === 'tower'}
    <path d="M9 2h15l2 28H7z" fill={icon.shell} /><path d="M12 4v23" stroke="#253149" stroke-width="3" /><path d="M17 7v12" stroke={icon.accent} stroke-width="1.5" /><path d="M16 24h4m-4 3h4" stroke="#a8bad2" /><circle cx="20" cy="4" r="1" fill={icon.accent} />
  {:else if icon.hardware === 'cube'}
    <path d="m16 2 13 7v16l-13 6-13-6V9z" fill={icon.shell} /><path d="m3 9 13 6 13-6M16 15v16" stroke="#4f477e" /><ellipse cx="16" cy="8" rx="6" ry="3" fill={icon.accent} /><circle cx="8" cy="19" r="1.5" fill="#263248" /><circle cx="12" cy="21" r="1.5" fill="#263248" />
  {:else if icon.hardware === 'xbox'}
    <rect x="2" y="6" width="28" height="23" rx="4" fill={icon.shell} /><path d="m5 9 22 16M27 9 5 25" stroke="#24372e" stroke-width="4" /><circle cx="16" cy="17" r="6" fill={icon.accent} /><path d="m13 14 6 6m0-6-6 6" stroke="#315144" stroke-width="2" />
  {:else if icon.hardware === 'arcade'}
    <path d="M7 1h18l-2 18 4 5v7H5v-7l4-5z" fill={icon.shell} /><rect x="9" y="4" width="14" height="11" rx="1" fill="#202b40" /><rect x="11" y="6" width="10" height="7" fill={icon.accent} /><path d="M11 19v4m-2 0h4" stroke="#263248" stroke-width="2" /><circle cx="20" cy="22" r="2" fill={icon.accent} /><rect x="13" y="27" width="6" height="2" fill="#263248" />
  {:else if icon.hardware === 'pc'}
    <rect x="1" y="4" width="22" height="18" rx="2" fill={icon.shell} /><rect x="3" y="6" width="18" height="12" rx="1" fill="#263248" /><path d="M5 9h14v6H5z" fill={icon.accent} fill-opacity=".7" /><path d="M12 22v5m-7 1h14" stroke={icon.shell} stroke-width="2" /><rect x="25" y="7" width="6" height="22" rx="1" fill={icon.shell} /><circle cx="28" cy="11" r="1" fill={icon.accent} />
  {:else}
    <rect x="2" y="9" width="28" height="18" rx="4" fill={icon.shell} /><rect x="9" y="6" width="14" height="7" rx="2" fill="#344158" /><path d="M11 9h10" stroke={icon.accent} stroke-width="2" /><rect x="5" y="20" width="7" height="3" rx="1" fill="#263248" /><rect x="20" y="20" width="7" height="3" rx="1" fill="#263248" />
  {/if}
</svg>

<style>.platform-icon { display: inline-block; flex: 0 0 auto; vertical-align: middle; overflow: visible; }</style>
