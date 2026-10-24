# Marquee

One console-style launcher for Fedora/Linux for your Steam, Epic, GOG, emulated, and native-port games. Browse one library with a mouse, keyboard, or controller, then choose an edition and a way to run it.

**Product direction (2026-10-22):** Marquee is the library and launcher. A hidden local **`marquee-manager`** owns custom downloads, torrents, source artifacts and preparation/install jobs, with progress and review in Marquee. Durable local-archive preparation, torrent history and the browser-job bridge run in that background process and continue after the launcher closes; the launcher registers completed receipts when it reconnects. A second full application is not required. Steam retains its official client; Epic uses Legendary and GOG a direct adapter plus gogdl, coordinated through store-specific contracts. Non-Steam runtime setup uses supported managed umu/Proton recipes. Lutris/Heroic remain optional. See the [manager architecture and implementation plan](docs/MARQUEE_MANAGER.md).

**Next implementation order:** remaining torrent/browser live-provider and package gates → console recipes → stores. Direct stores retain Steam → compatibility/Epic → GOG order after that foundation. Read-only [Steam inventory](docs/STORE_INTEGRATION.md#steam-inventory-implementation) already exists and may continue independently. See the [roadmap](docs/ROADMAP.md), [manager gates](docs/MARQUEE_MANAGER.md#implementation-sequence-and-gates), and [store handoff](docs/STORE_INTEGRATION.md).

**Current working tree:** Rust core/CLI, Tauri/Svelte desktop and shared browser surface; PS2/PCSX2, PS1/DuckStation slice, serial covers, session/correction work, five managed native-port recipes, unified-library UI, external launcher shortcuts, and a legacy read-only Lutris bridge. A narrow [Purple for Aion 2](docs/PURPLE.md) integration now automates its umu/GE-Proton setup and launcher opening; actual Purple/Aion 2 Linux compatibility remains unverified. Direct Steam/Epic/GOG integrations and general Windows-app installation are not implemented end to end. Other “Add a Windows app” recipes remain demo-only. Real emulator launch/exit acceptance is tracked in [PS2](docs/PS2.md) and [PS1](docs/PS1.md); demo/fake-runner tests do not close those gates. Existing Lutris cache projection does not count as a direct store connection.

**Emulation and metadata:** PS1/PS2 disc adapters are joined by Nintendo DS/melonDS, Genesis/BlastEm and Neo Geo/FinalBurn Neo library adapters. melonDS, BlastEm, FBNeo and mGBA have pinned local source recipes using rootless Podman. ZIP/7z/RAR [archive intake](docs/INTAKE.md) extracts cartridges/discs and preserves arcade sets. BIOS import/list/removal remains available for PS1/PS2. Live checks reached NFS Underground's PS2 executable, Pokémon Platino's DS intro and Streets of Rage 2's Genesis intro; full controller/save acceptance remains open. The supplied incomplete Neo Geo set is blocked with a diagnostic. See [emulator management](docs/EMULATORS.md).

Saved Vimm catalogue records retain [field provenance and release identity](docs/CATALOGUE_METADATA.md). Optional [ROM enrichment](docs/ENRICHMENT.md#implemented-artwork-and-facts) supplies covers and game facts from libretro and configured providers. Title-based enrichment is not a verified release mapping, and artwork credit does not establish shared provider IDs. General field overrides and verified cross-provider mappings remain follow-up work.

Documentation navigation: [docs index](docs/README.md). Broader platform/port ideas remain in the [expansion catalogue](docs/EXPANSION_CATALOGUE.md); [Jak/OpenGOAL and edition/run choices](docs/UNIFIED_LIBRARY.md) remain a separate track. Metadata matching and [ScreenScraper/SteamGridDB](docs/ENRICHMENT.md#ps2-metadata-matching-and-cached-artwork) must never gate Play. **Media goal:** `marquee-manager` acquires and prepares supported content from configured authorized sources; Marquee registers validated installation receipts and launches games. Downloaded bytes are not an installation. Today, local archive preparation, torrent transport and the browser-job bridge have saved reviews/jobs/receipts and library registration; remaining live-provider and unsupported-package gates are open. Folder import stays available. See [media acquisition](docs/MARQUEE_MANAGER.md#acquisition-user-flow).

The owner has also added **Switch, PS3, PS4, original Xbox and Xbox 360 games**. These are collection requirements, not completed adapters: RPCS3/shadPS4/xemu have tool entries; PS3 has read-only discovery and reviewed firmware setup, but none of the five have Library Play. Their sources are inventoried under ignored `sample_images/`; license/key contents are not imported. See the [console preparation plan](docs/CONSOLE_PREPARATION.md).

## Run

**Native streaming:** Wolf API controls are available through `marquee streaming
--socket /absolute/path/wolf.sock` (inspect, pending, pair, unpair, stop). Native
`streaming build`, `runtime-status`, `probe` and `rollback` manage a pinned local
source build with software/NVIDIA checks and image receipts. Host lifecycle
passed locally; an end-to-end game stream, automatic hosting lifecycle, UI and
integrated player remain upcoming. See [source reuse, commands and gates](docs/STREAMING.md).

Needs Node 22+, pnpm (`corepack enable pnpm`), and Rust for the desktop/CLI. On Fedora, install WebKit and the other Tauri devel packages first:

```bash
./scripts/setup.sh                # Fedora devel packages, then checks rustc / node / pnpm
(cd apps/desktop && pnpm install)
./scripts/run.sh desktop          # optimized app, built when needed; no Vite/web server
./scripts/run.sh dev              # explicit browser development with hot reload
```

Desktop rendering automatically uses an accelerated shared-memory transport on NVIDIA/Wayland. `MARQUEE_RENDERER=software ./scripts/run.sh desktop` retains the compatibility fallback. See [rendering modes, production launch and live benchmark results](docs/DESKTOP_RENDERING.md).

CLI (same core; use `--data-dir` for an isolated database):

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps2 --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners install --channel auto
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev artwork refresh
cargo build -p marquee-manager -p marquee-cli        # scripts/run.sh builds the companion automatically
cargo run -p marquee-cli -- prepare inspect /absolute/path/Game.zip --system genesis
cargo run -p marquee-cli -- prepare run JOB_ID --review-token TOKEN_FROM_INSPECTION
cargo run -p marquee-cli -- prepare start JOB_ID --review-token TOKEN_FROM_INSPECTION  # detach
cargo run -p marquee-cli -- prepare list             # register pending receipts
cargo run -p marquee-cli -- manager status           # also: manager start | stop
cargo run -p marquee-cli -- intake "Game (USA).zip"    # legacy immediate importer
cargo run -p marquee-cli -- emulators build blastem   # Linux x86_64; Git + rootless Podman
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev ports install zelda64recomp   # also: ports list | check | launch
```

**One app, two surfaces, run separately.** The desktop app is standalone and opens no port by default. Set `MARQUEE_WEB=1` (or `MARQUEE_PORT=<port>`) and it also serves the same UI and library on `127.0.0.1` (default port 47820). For the web alone, run `scripts/run.sh web` (`marquee run web`) with no window. Settings → Web access copies an address ending in `#token=…`; open it in a browser on the same computer and you get the identical UI with the same games, covers, ports and launches, and both surfaces see each other's changes live. Every UI build is stamped; a window or tab left open across a rebuild shows a “Reload” banner instead of silently running old code. Headless: `marquee run web` (after `pnpm build`). While developing, `scripts/run.sh dev` waits for a healthy backend and opens the authenticated Vite URL (`http://localhost:1420/#token=…`). It proxies API and artwork to the backend's actual selected port, reuses a running web-enabled desktop for the same library, and honors `MARQUEE_DATA_DIR` (or `--data-dir PATH`) instead of opening another database. A desktop started without web access must be closed or relaunched with `scripts/run.sh both`; an occupied library now stops dev before Vite starts. `MARQUEE_DEV_PORT` changes Vite's port; `MARQUEE_API_URL` can select an existing loopback backend outside the default port range. The server is loopback-only, needs the token for every API and cover request, and rejects foreign `Host` headers. The token lives in `<data dir>/server.token` (mode 600).

**Demo (`pnpm dev`, no token, or `?demo=1`)** still uses invented titles and simulated playtime. Controller input in the browser is not evidence for the Tauri window.

**Desktop** uses the real library and the same Home/Games/Music/Tools/Streaming/Settings chrome as the demo. With no imported media or enabled source it is empty; the optional legacy Lutris bridge stays disconnected until explicitly enabled. Install PCSX2 from Home, Library, or Settings: Marquee downloads the official AppImage and unpacks it into its own data dir (no Flatpak wizard). Point library roots at your disc folders (automated acquisition jobs are the target); covers are looked up by disc serial and cached. **Native ports** appear as tagged games, with engine installation/configuration in Tools; retail data should eventually flow from store/detection jobs, with **Open folder** as fallback today. **Tools → Purple for Aion 2** provides managed setup and opening with progress, stop/retry and private diagnostics; Purple performs account sign-in, game downloads and patching. This is an unverified Linux compatibility recipe. A persistent Downloads screen is not implemented; Purple progress is on its Tools card. Other Windows-app recipes remain demo-only.

Dev URL params: `?route=library`, `?mode=big`, `?theme=light`, `?game=<id suffix>`, `?add=1`, `?splash=0`, `?motion=0`, `?empty=1` (demo with no sample games, for the empty desktop look).

Before pushing, run `scripts/check.sh` (there is no hosted CI). It formats and tests the Rust core/CLI, compiles the desktop crate (prints `BLOCKED` if WebKit/DBus devel are missing; **fails** if they are present and compile errors), then `pnpm check` / `pnpm test` / `pnpm build`. Frontend-only: `pnpm test` inside `apps/desktop/`.

## Layout

```
apps/desktop/   Svelte 5 + TypeScript + Vite UI and Tauri 2 shell
crates/core/    library DB/receipt import, scan, launch/session adapters, command layer, events, web server
crates/manager/ hidden preparation worker, private socket/client, jobs/receipts and ownership migration
crates/media/   shared bounded archive/CUE/ISO helpers, catalogue and errors
crates/cli/     `marquee` diagnostics over the same core
catalog/        PS2/PS1 system descriptors, ports/ (one JSON per port)
scripts/        setup.sh, run.sh, check.sh, make-icon.py
docs/           ROADMAP.md, STORE_INTEGRATION.md, PS2.md, PS1.md, UNIFIED_LIBRARY.md, and supporting notes
```

## Secrets

Keys (SteamGridDB, Steam Web API, SteamID, IGDB/Twitch, RAWG) are managed in **Settings → API keys**, in the native app and in the browser surface alike. A value is read from, in order: what you saved in Marquee, the process environment, then a `.env` file (see `.env.example`; git-ignored). Saved values live in `<config dir>/keys.json`, readable only by you, and the UI only ever sees where a key comes from and its last four characters, never the value. Adding a key to Marquee is one entry in `crates/core/src/keys.rs`. Never put a Steam password anywhere: Marquee launches through the Steam client, which keeps its own login. Cover refresh uses a SteamGridDB key when configured; IGDB and RAWG remain unused infrastructure for later artwork and Steam extras.

## License

[MIT](LICENSE).

## Unified library and correction pass

See [the unified-library model, Jak/OpenGOAL example, and next implementation milestone](docs/UNIFIED_LIBRARY.md) and [the correction report](docs/CORRECTIONS.md). The browser demo now includes a Jak edition/run selector; the real library includes installed native ports alongside discs.

## UI and navigation

See the [UI/UX implementation record and remaining visual acceptance](docs/UI_UX.md). Home contains personal library entries; catalogue records have their own discovery view. Installed port engines are labeled as needing unchecked game setup rather than automatically Ready to play. Settings groups everyday preferences separately from provider keys and library maintenance.

## Tools and launchers

The desktop defaults to **Balanced** visual effects to avoid large blur and grain passes. Settings → Look and feel offers Full effects and Reduce motion. See the [performance findings, fixes and remaining native profiling checks](docs/DESKTOP_RENDERING.md).

**Tools → Emulators → mGBA** now offers a pinned source-build pilot with progress, logs, cancellation/retry and previous-version rollback. Compiler and runtime dependencies stay in rootless Podman; GB/GBC/GBA library import remains follow-up work. See [source builds and verification](docs/EMULATORS.md#source-builds-mgba-pilot) and [game-only/headless modes](docs/EMULATORS.md#game-only-windows-and-headless-emulation).

The target normal flow detects and manages store helpers automatically. The current custom shortcut editor is an advanced feature, not the planned account-connection workflow. Use **Tools → Add launcher** to configure existing apps, AppImages, or native runtimes. Saved launchers support individual arguments, a working directory, readiness checks, and pinning. Steam/Lutris/Heroic command detection is included. See [launcher setup, supported behavior, and verification](docs/LAUNCHERS.md).

## Legacy Lutris bridge

The read-only Lutris bridge is explicitly optional. Tools → Legacy integrations reveals its connection settings; a fresh setup never reads its game database or imports cached store listings automatically. Existing explicitly saved connections remain enabled, and disconnecting removes only the projected view. Available database paths can be suggested without opening them. No Lutris files, prefixes or credentials are changed. No further Lutris setup/import work is required for the direct-store milestones. See [bridge behavior and its superseded priority](docs/LAUNCHERS.md#legacy-lutris-library-bridge).
