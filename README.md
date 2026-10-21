# Marquee

One console-style launcher for Fedora/Linux for your Steam, Epic, GOG, emulated, and native-port games. Browse one library with a mouse, keyboard, or controller, then choose an edition and a way to run it.

**Product direction (2026-10-21):** Marquee replaces the user's need for a separate library manager. It owns store connections, library sync, install/update jobs, compatibility setup, runtime/prefix selection, and the normal Play flow. Steam uses its official client underneath; Epic uses Legendary; GOG uses a direct adapter and gogdl; non-Steam Windows games use managed umu/Proton. Lutris and Heroic are optional legacy integrations, not prerequisites. These direct store/runtime workflows are the target, not completed features.

**Next implementation order:** direct Steam integration → managed compatibility plus Epic/Legendary → GOG/gogdl. Installed Steam discovery is the first usable increment; authorized owned-library sync and installation handoff complete the Steam milestone. See the [roadmap](docs/ROADMAP.md) and [step-by-step implementation handoff](docs/STORE_INTEGRATION.md).

**Current working tree:** Rust core/CLI, Tauri/Svelte desktop and shared browser surface; PS2/PCSX2, PS1/DuckStation slice, serial covers, session/correction work, five managed native-port recipes, unified-library UI, external launcher shortcuts, and a legacy read-only Lutris bridge. A narrow [Purple for Aion 2](docs/PURPLE.md) integration now automates its umu/GE-Proton setup and launcher opening; actual Purple/Aion 2 Linux compatibility remains unverified. Direct Steam/Epic/GOG integrations and general Windows-app installation are not implemented end to end. Other “Add a Windows app” recipes remain demo-only. Real emulator launch/exit acceptance is tracked in [PS2](docs/PS2.md) and [PS1](docs/PS1.md); demo/fake-runner tests do not close those gates. Existing Lutris cache projection does not count as a direct store connection.

**Tools additions:** managed emulator installation/opening/update/removal for DuckStation, PPSSPP, RPCS3, Flycast, Azahar, Cemu and xemu; BIOS import/list/removal for PS1/PS2. DuckStation connects to PS1 library launch; the other six currently open independently. PCSX2 uses its existing dedicated installer. Catalogue availability does not establish tested game compatibility. See [emulator management](docs/EMULATORS.md) and the [PS1/PS2 correction pass](docs/PS1_PS2_CORRECTIONS_2026-10-21.md).

Documentation navigation: [docs index](docs/README.md). Broader platform/port ideas remain in the [expansion catalogue](docs/EXPANSION_CATALOGUE.md); [Jak/OpenGOAL and edition/run choices](docs/UNIFIED_LIBRARY.md) remain a separate track. Metadata matching and [ScreenScraper/SteamGridDB](docs/PS2_METADATA_FOLLOWUP.md) must never gate Play. Original ROM/BIOS/firmware/port extraction assets remain user-supplied; authenticated downloads of owned store games are in scope.

## Run

Needs Node 22+, pnpm (`corepack enable pnpm`), and Rust for the desktop/CLI. On Fedora, install WebKit and the other Tauri devel packages first:

```bash
./scripts/setup.sh                # Fedora devel packages, then checks rustc / node / pnpm
(cd apps/desktop && pnpm install)
./scripts/run.sh desktop          # or: web | dev | both | tauri  (also pnpm run:desktop …)
(cd apps/desktop && pnpm dev)     # browser demo with sample data · http://localhost:1420
```

CLI (same core; use `--data-dir` for an isolated database):

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps2 --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners install --channel auto
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev artwork refresh
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev ports install zelda64recomp   # also: ports list | check | launch
```

**One app, two surfaces, run separately.** The desktop app is standalone and opens no port by default. Set `MARQUEE_WEB=1` (or `MARQUEE_PORT=<port>`) and it also serves the same UI and library on `127.0.0.1` (default port 47820). For the web alone, run `scripts/run.sh web` (`marquee run web`) with no window. Settings → Web access copies an address ending in `#token=…`; open it in a browser on the same computer and you get the identical UI with the same games, covers, ports and launches, and both surfaces see each other's changes live. Headless: `marquee run web` (after `pnpm build`). While developing, `scripts/run.sh dev` starts the server and Vite together; Vite proxies `/api` to a running Marquee, so a browser at `http://localhost:1420/#token=…` shows real data with hot reload. The server is loopback-only, needs the token for every API and cover request, and rejects foreign `Host` headers. The token lives in `<data dir>/server.token` (mode 600).

**Demo (`pnpm dev`, no token, or `?demo=1`)** still uses invented titles and simulated playtime. Controller input in the browser is not evidence for the Tauri window.

**Desktop** uses the real library and the same Home/Library/Tools/Ports/Downloads/Settings chrome as the demo. With no imported media or enabled source it is empty; existing code can also auto-discover a legacy Lutris library, pending the opt-in change described below. Install PCSX2 from Home, Library, or Settings: Marquee downloads the official AppImage and unpacks it into its own data dir (no Flatpak wizard, no BIOS or games). Add a PS2 `.iso` folder; covers are looked up by disc serial and cached. **Ports** installs five native ports from their official GitHub releases (Ship of Harkinian, Zelda64Recomp, sm64coopdx, Fallout 2 CE, UZDoom): Install, Update, Play, and Open folder for your own ROM or game files. **Tools → Purple for Aion 2** provides managed setup and opening with progress, stop/retry and private diagnostics; Purple performs account sign-in, game downloads and patching. This is an unverified Linux compatibility recipe. Downloads on desktop is still empty chrome; Purple progress is on its Tools card. Other Windows-app recipes remain demo-only.

Dev URL params: `?route=library`, `?mode=big`, `?theme=light`, `?game=<id suffix>`, `?add=1`, `?splash=0`, `?motion=0`, `?empty=1` (demo with no sample games, for the empty desktop look).

Before pushing, run `scripts/check.sh` (there is no hosted CI). It formats and tests the Rust core/CLI, compiles the desktop crate (prints `BLOCKED` if WebKit/DBus devel are missing; **fails** if they are present and compile errors), then `pnpm check` / `pnpm test` / `pnpm build`. Frontend-only: `pnpm test` inside `apps/desktop/`.

## Layout

```
apps/desktop/   Svelte 5 + TypeScript + Vite UI and Tauri 2 shell
crates/core/    library DB, ISO scan, PCSX2 unpack/argv, serial covers, sessions, ports, command layer (`api`), event hub, local web server
crates/cli/     `marquee` diagnostics over the same core
catalog/        PS2/PS1 system descriptors, ports/ (one JSON per port)
scripts/        setup.sh, run.sh, check.sh, make-icon.py
docs/           ROADMAP.md, STORE_INTEGRATION.md, PS2.md, PS1.md, UNIFIED_LIBRARY.md, and supporting notes
```

## Secrets

Keys (SteamGridDB, Steam Web API, SteamID, IGDB/Twitch, RAWG) are managed in **Settings → API keys**, in the native app and in the browser surface alike. A value is read from, in order: what you saved in Marquee, the process environment, then a `.env` file (see `.env.example`; git-ignored). Saved values live in `<config dir>/keys.json`, readable only by you, and the UI only ever sees where a key comes from and its last four characters, never the value. Adding a key to Marquee is one entry in `crates/core/src/keys.rs`. Never put a Steam password anywhere: Marquee launches through the Steam client, which keeps its own login. Nothing uses these keys yet; they are the infrastructure for artwork and the Steam extras.

## License

[MIT](LICENSE).

## Unified library and correction pass

See [the unified-library model, Jak/OpenGOAL example, and next implementation milestone](docs/UNIFIED_LIBRARY.md) and [the correction report](docs/CORRECTION_PASS.md). The browser demo now includes a Jak edition/run selector; the real library includes installed native ports alongside discs.

## Tools and launchers

The desktop defaults to **Balanced** visual effects to avoid large blur and grain passes. Settings → Look and feel offers Full effects and Reduce motion. See the [performance findings, fixes and remaining native profiling checks](docs/DESKTOP_PERFORMANCE.md).

**Tools → Emulators → mGBA** now offers a pinned source-build pilot with progress, logs, cancellation/retry and previous-version rollback. Compiler and runtime dependencies stay in rootless Podman; GB/GBC/GBA library import remains follow-up work. See [source builds and verification](docs/SOURCE_BUILDS.md) and [game-only/headless modes](docs/EMULATOR_PRESENTATION.md).

The target normal flow detects and manages store helpers automatically. The current custom shortcut editor is an advanced feature, not the planned account-connection workflow. Use **Tools → Add launcher** to configure existing apps, AppImages, or native runtimes. Saved launchers support individual arguments, a working directory, readiness checks, and pinning. Steam/Lutris/Heroic command detection is included. See [launcher setup, supported behavior, and verification](docs/LAUNCHERS.md).

## Legacy Lutris bridge

Existing code automatically reads a detected Lutris library and its cached store listings. This is legacy behavior from the earlier approach; the plan is to make it explicitly optional and remove it from default onboarding. This documentation update does not disable or delete that code or any user data. No further Lutris setup/import work is required for the direct-store milestones. See [bridge behavior and its superseded priority](docs/AUTOMATIC_SOURCES.md).
