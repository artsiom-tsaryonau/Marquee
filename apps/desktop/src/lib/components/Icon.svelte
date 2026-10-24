<script lang="ts" module>
  // Line icons on a 24px grid, stroke = currentColor.
  const paths = {
    home: 'M3 11.5 12 4l9 7.5M5.5 9.5V20h13V9.5',
    grid: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z',
    puzzle: 'M9 4h3a2 2 0 1 1 4 0h0v4h0a2 2 0 1 1 0 4v4h-4a2 2 0 1 0-4 0H4v-4a2 2 0 1 0 0-4V4z',
    download: 'M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 19h14',
    settings: 'M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4zM19.4 13.5l1.6 1.2-2 3.4-1.9-.7a7 7 0 0 1-2 1.2L14.8 21h-4l-.3-2.4a7 7 0 0 1-2-1.2l-2 .7-2-3.4 1.7-1.2a7 7 0 0 1 0-3l-1.7-1.2 2-3.4 2 .7a7 7 0 0 1 2-1.2L10.8 3h4l.3 2.4a7 7 0 0 1 2 1.2l1.9-.7 2 3.4-1.6 1.2a7 7 0 0 1 0 3z',
    play: 'M8 5.5v13l10.5-6.5z',
    search: 'M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM20 20l-4.8-4.8',
    star: 'm12 3.8 2.5 5.2 5.7.8-4.1 4 1 5.6-5.1-2.7-5.1 2.7 1-5.6-4.1-4 5.7-.8z',
    clock: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3.5 2',
    folder: 'M3.5 6.5h6l2 2h9v10.5h-17z',
    image: 'M4 5h16v14H4zM4 16l5-5 4 4 2.5-2.5L20 17M15.5 9.5h.01',
    edit: 'm15 4 5 5M4 20l5-1L21 7a2 2 0 0 0-5-5L4 14z',
    plus: 'M12 5v14M5 12h14',
    check: 'm5 12.5 4.5 4.5L19 7.5',
    alert: 'M12 4 2.8 20h18.4zM12 10v4.5M12 17.5h.01',
    controller: 'M7 8h10a4 4 0 0 1 4 4.5l-.6 3.6a2.4 2.4 0 0 1-4.2 1.1L14.5 15h-5l-1.7 2.2a2.4 2.4 0 0 1-4.2-1.1L3 12.5A4 4 0 0 1 7 8zM8 10.5v3M6.5 12h3M15.5 11h.01M17.5 13h.01',
    tv: 'M3.5 5.5h17v11h-17zM8 20h8M12 16.5V20',
    monitor: 'M3.5 4.5h17v12h-17zM9 20h6M12 16.5V20',
    back: 'M15 5l-7 7 7 7',
    sun: 'M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM12 2.5v2M12 19.5v2M4.6 4.6 6 6M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4',
    moon: 'M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z',
    pause: 'M7 5h4v14H7zM13 5h4v14h-4z',
    previous: 'M5 5h2v14H5zM19 5 8 12l11 7z',
    next: 'M17 5h2v14h-2zM5 5l11 7-11 7z',
    volume: 'M4 9v6h4l5 4V5L8 9zM16 8a6 6 0 0 1 0 8M19 5a10 10 0 0 1 0 14',
    muted: 'M4 9v6h4l5 4V5L8 9zM17 9l5 6M22 9l-5 6',
    more: 'M5 12h.01M12 12h.01M19 12h.01',
    queue: 'M4 5h16M4 10h16M4 15h8M16 14l5 3-5 3z',
    music: 'M9 18V5l11-2v13M9 8l11-2M9 18a3 2 0 1 1-3-2c1.7 0 3 .9 3 2zM20 16a3 2 0 1 1-3-2c1.7 0 3 .9 3 2z',
    refresh: 'M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4v4h-4',
    x: 'M6 6l12 12M18 6 6 18',
    sparkle: 'M12 3c.6 4.6 2.4 6.4 7 7-4.6.6-6.4 2.4-7 7-.6-4.6-2.4-6.4-7-7 4.6-.6 6.4-2.4 7-7z',
    chevron: 'M6 9l6 6 6-6',
    share: 'M12 15V4m0 0L8 8m4-4 4 4M5 13v6h14v-6',
    info: 'M12 11.5v5M12 8h.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z',
  } as const;
  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name, size = 20, filled = false }: { name: IconName; size?: number; filled?: boolean } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} fill={filled || name === 'play' ? 'currentColor' : 'none'} stroke={name === 'play' || filled && ['pause', 'previous', 'next'].includes(name) ? 'none' : undefined} />
</svg>
