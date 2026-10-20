# Marquee

One console-style launcher for Fedora that shows every game you own (Steam, GOG, Epic, Lutris/Wine apps, emulated games and native decomp/recomp ports) in one good-looking library you can drive with a mouse or a controller.

Marquee is a front end over existing tools. Lutris, Steam, Legendary, umu-launcher/Proton and the emulators keep doing installs and compatibility; Marquee owns the library, the look, artwork, the ports catalogue and launching. Windows launchers install from recipes, so there is no manual Wine setup. Game pages pull scores, time to beat and achievements from IGDB, Steam reviews, RAWG, ScreenScraper, RetroAchievements and PCGamingWiki, with link-outs to SteamDB, Metacritic and HowLongToBeat.

**Status (2026-10-20):** current goal is to **solidify the PS2 backend and desktop app** (library DB, ISO import, **Marquee-managed PCSX2 unpack**, serial covers, sessions, UI chrome). The native window **opens** and uses the same look as `pnpm dev`. Empty desktop never shows sample games. **A real PCSX2 Play → quit → duration has not been verified** on the implementation host. Steam/Lutris adapters are not started. LICENSE is unchosen.

PS2 slice (status, setup, verification): [docs/PS2.md](docs/PS2.md). Long-term plan: [docs/ROADMAP.md](docs/ROADMAP.md). Other systems: [expansion catalogue](docs/EXPANSION_CATALOGUE.md).

Do not start Steam/Lutris or [ScreenScraper / SteamGridDB](docs/PS2_METADATA_FOLLOWUP.md) until the PCSX2 launch/exit gate in [docs/PS2.md](docs/PS2.md) is closed. Serial covers are already in this slice; they never download BIOS or games.

## Run

Needs Node 22+, pnpm (`corepack enable pnpm`), and Rust for the desktop/CLI. On Fedora, install WebKit and the other Tauri devel packages first:

```bash
./scripts/fedora-deps.sh          # webkit2gtk4.1-devel, dbus-devel, …
./scripts/bootstrap.sh            # same, then checks rustc / node / pnpm
cd apps/desktop
pnpm install
pnpm dev              # browser demo with sample data · http://localhost:1420
pnpm tauri dev        # real library (empty until you add a PS2 ISO folder)
# NVIDIA + Wayland:
./scripts/tauri-dev.sh
```

CLI (same core; use `--data-dir` for an isolated database):

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps2 --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners install --channel auto
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev artwork refresh
```

**Demo (`pnpm dev`)** still uses invented titles and simulated playtime. Controller input in the browser is not evidence for the Tauri window.

**Desktop** starts with an empty library and the same Home/Library/Ports/Downloads/Settings chrome as the demo. Install PCSX2 from Home, Library, or Settings: Marquee downloads the official AppImage and unpacks it into its own data dir (no Flatpak wizard, no BIOS or games). Add a PS2 `.iso` folder; covers are looked up by disc serial and cached. Ports and Downloads on desktop are empty chrome, not a ports manager. “Add a Windows app” is demo-only.

Dev URL params: `?route=library`, `?mode=big`, `?theme=light`, `?game=<id suffix>`, `?add=1`, `?splash=0`, `?motion=0`, `?empty=1` (demo with no sample games, for the empty desktop look).

Before pushing, run `scripts/check.sh` (there is no hosted CI). It formats and tests the Rust core/CLI, compiles the desktop crate (prints `BLOCKED` if WebKit/DBus devel are missing; **fails** if they are present and compile errors), then `pnpm check` / `pnpm test` / `pnpm build`. Frontend-only: `pnpm test` inside `apps/desktop/`.

## Layout

```
apps/desktop/   Svelte 5 + TypeScript + Vite UI and Tauri 2 shell
crates/core/    library DB, ISO scan, PCSX2 unpack/argv, serial covers, sessions
crates/cli/     `marquee` diagnostics over the same core
catalog/        built-in PS2 system descriptor
scripts/        fedora-deps.sh, bootstrap.sh, check.sh, make-icon.py, tauri-dev.sh
docs/           PS2.md, ROADMAP.md, EXPANSION_CATALOGUE.md, PS2_METADATA_FOLLOWUP.md
```

## License

Not chosen yet (GPL-3.0-or-later or MIT); see the open decisions in the roadmap.
