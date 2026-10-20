# Marquee

One console-style launcher for Fedora that shows every game you own (Steam, GOG, Epic, Lutris/Wine apps, emulated games and native decomp/recomp ports) in one good-looking library you can drive with a mouse or a controller.

Marquee is a front end over existing tools. Lutris, Steam, Legendary, umu-launcher/Proton and the emulators keep doing installs and compatibility; Marquee owns the library, the look, artwork, the ports catalogue and launching. Windows launchers install from recipes, so there is no manual Wine setup. Game pages pull scores, time to beat and achievements from IGDB, Steam reviews, RAWG, ScreenScraper, RetroAchievements and PCGamingWiki, with link-outs to SteamDB, Metacritic and HowLongToBeat.

**Status (2026-10-20):** the offline PS2 slice is **in the tree** (core, CLI, Tauri shell, Svelte desktop/demo split). The browser `pnpm dev` demo still uses sample data. **A real PCSX2 launch/exit has not been verified** on the implementation host. Steam/Lutris adapters are not started.

PS2 slice (status, setup, verification): [docs/PS2.md](docs/PS2.md). Long-term plan: [docs/ROADMAP.md](docs/ROADMAP.md). Other systems: [expansion catalogue](docs/EXPANSION_CATALOGUE.md).

Do not start Steam/Lutris or [metadata/artwork](docs/PS2_METADATA_FOLLOWUP.md) until the PCSX2 launch/exit gate in [docs/PS2.md](docs/PS2.md) is closed.

## Run

Needs Node 22+, pnpm (`corepack enable pnpm`), and Rust for the desktop/CLI. On Fedora, install WebKit and the other Tauri devel packages first:

```bash
./scripts/fedora-deps.sh          # webkit2gtk4.1-devel, dbus-devel, …
./scripts/bootstrap.sh            # same, then checks rustc / node / pnpm
cd apps/desktop
pnpm install
pnpm dev          # browser demo with sample data · http://localhost:1420
pnpm tauri dev    # real library (empty until you add a PS2 ISO folder)
```

CLI (same core; use `--data-dir` for an isolated database):

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
```

**Demo (`pnpm dev`)** still uses invented titles and simulated playtime. Controller input in the browser is not evidence for the Tauri window.

**Desktop (`pnpm tauri dev`)** starts with an empty library. Add a PS2 `.iso` folder from Settings or the sidebar. Artwork is procedural until the metadata follow-up. “Add a Windows app”, Ports and Downloads are demo-only.

Dev URL params (demo): `?route=library`, `?mode=big`, `?theme=light`, `?game=<id suffix>`, `?add=1`, `?splash=0`, `?motion=0`.

Before pushing, run `scripts/check.sh` (there is no hosted CI). It formats and tests the Rust core/CLI, attempts a desktop compile (prints `BLOCKED` and points at `scripts/fedora-deps.sh` if WebKit/DBus devel are missing), then `pnpm check` / `pnpm test` / `pnpm build`. Frontend-only: `pnpm test` inside `apps/desktop/`.

## Layout

```
apps/desktop/   Svelte 5 + TypeScript + Vite UI and Tauri 2 shell
crates/core/    library DB, ISO scan, PCSX2 argv, sessions
crates/cli/     `marquee` diagnostics over the same core
catalog/        built-in PS2 system descriptor
scripts/        fedora-deps.sh, bootstrap.sh, check.sh
docs/           PS2.md, ROADMAP.md, EXPANSION_CATALOGUE.md, PS2_METADATA_FOLLOWUP.md
```

## License

Not chosen yet (GPL-3.0-or-later or MIT); see the open decisions in the roadmap.
