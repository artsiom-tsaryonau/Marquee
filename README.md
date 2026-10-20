# Marquee

One console-style launcher for Fedora that shows every game you own (Steam, GOG, Epic, Lutris/Wine apps, emulated games and native decomp/recomp ports) in one good-looking library you can drive with a mouse or a controller.

Marquee is a front end over existing tools. Lutris, Steam, Legendary, umu-launcher/Proton and the emulators keep doing installs and compatibility; Marquee owns the library, the look, artwork, the ports catalogue and launching. Windows launchers install from recipes, so there is no manual Wine setup. Game pages pull scores, time to beat and achievements from IGDB, Steam reviews, RAWG, ScreenScraper, RetroAchievements and PCGamingWiki, with link-outs to SteamDB, Metacritic and HowLongToBeat.

**Status:** phase 0. The UI runs in a browser with sample data; nothing reads your real library yet. Plan and phases: [docs/ROADMAP.md](docs/ROADMAP.md).

## Run the UI mock-up

Needs Node 22+ and pnpm (`corepack enable pnpm`).

```bash
cd apps/desktop
pnpm install
pnpm dev          # http://localhost:1420
```

What to try:

- Arrow keys move focus like a D-pad, Enter selects, Esc goes back. A connected gamepad works too (A/B, D-pad, left stick).
- On Library, start typing anywhere to search.
- Settings → Mode → Big Screen for the couch layout; Theme → Light.
- The **+** in the sidebar opens "Add a Windows app" (the recipe flow that replaces manual Wine setup).
- Dev URL params: `?route=library`, `?mode=big`, `?theme=light`, `?game=<id suffix>`, `?add=1`, `?splash=0`, `?motion=0`.

All artwork is generated on the fly (`src/lib/components/Artwork.svelte`); library titles are invented.

## Layout

```
apps/desktop/   Svelte 5 + TypeScript + Vite UI (Tauri shell comes next)
docs/           ROADMAP.md
```

## License

Not chosen yet (GPL-3.0-or-later or MIT); see the open decisions in the roadmap.
