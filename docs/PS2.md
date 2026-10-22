# PlayStation 2 (offline slice)

Current correction notes: [PS1/PS2 correction pass](CORRECTIONS.md). Platform mapping, CUE handling and launch access have changed since the historical checks below. Live gameplay acceptance remains separate.

Slice notes from 2026-10-20; work-order clarification updated 2026-10-21.

This document records the PS2 slice: **what exists**, **how to run it**, **what was verified**, and **what is next**. Long-term phases: [ROADMAP.md](ROADMAP.md). Other systems: [EXPANSION_CATALOGUE.md](EXPANSION_CATALOGUE.md). Next ticket (not started): [PS2_METADATA_FOLLOWUP.md](ENRICHMENT.md#ps2-metadata-matching-and-cached-artwork).

Release boundary:

> A persistent, offline PS2 library that launches through an existing PCSX2 installation, observes its exit, and records actual session duration.

That gate is **not closed**. Core, CLI, and desktop UI are in the tree; a real PCSX2 Play → quit → duration has not been observed on the implementation host. The owner has reordered work: direct Steam → Epic/Legendary → GOG/gogdl may proceed while this live gate remains open. ScreenScraper remains its own follow-up; Lutris is not a core dependency. See [ROADMAP.md](ROADMAP.md).

Marquee does not download BIOS files or commercial games.

---

## Current state

| Surface | State |
| --- | --- |
| Web (`marquee run web`) | **Works.** Same dispatch layer as the desktop shell, token-authenticated, no sample data. |
| CLI (`marquee`) | **Works** without GTK. Same core as desktop. Use `--data-dir` for isolation. |
| Desktop (`./scripts/run.sh desktop`) | Optimized standalone window with embedded UI. NVIDIA/Wayland now defaults to accelerated shared-memory transport; see [live rendering correction](DESKTOP_RENDERING.md). Software mode is an explicit fallback. Native folder-picker / Play clicks are **not live-tested**. |
| Persistent library | SQLite schema **v4** after the shared PS1 root-system work (artwork v2, session recovery v3) at `$XDG_DATA_HOME/marquee/library.db` (or `--data-dir` / `MARQUEE_DATA_DIR`). `artwork` table + migrate v1→2. Newer `user_version` is rejected. |
| In-app PCSX2 install | **Built, via the emulator catalogue.** Downloads the official AppImage, verifies it, unpacks it into `$XDG_DATA_HOME/marquee/runners/pcsx2/current`. Flatpak, keep-as-AppImage and the dedicated installer were removed 2026-10-21; host copies are never discovered. First launch seeds `PCSX2.ini` so no setup wizard appears (see [EMULATORS](EMULATORS.md)). |
| Serial covers | **Built.** xlenore/ps2-covers JPEG cache, `convertFileSrc` + asset protocol. ScreenScraper/SteamGridDB still [follow-up](ENRICHMENT.md#ps2-metadata-matching-and-cached-artwork). |
| Real PCSX2 Play → quit → duration | **Pending.** Fake runner `/bin/sleep` records duration in tests. That is not the gate. |
| Controller in Tauri WebView | **Pending.** Browser Gamepad API + focus engine exist; `gilrs` not added. Browser gamepad is not evidence. |

`origin/main` as of this sweep: `e902f4e` (WebKit/DBus Fedora script). ISO library + Tauri shell: `1b20257`. **Unpack install, covers, icons, schema v2, isolated tests, desktop look, `run.sh`, `check.sh` compile-fail, and the 2026-10-20 roadmap/catalogue sweep are in the working tree, not pushed.** Commit when you say so.

### In scope for this slice

ISO import, PCSX2 discovery/argv, one supervised session, real playtime, offline UI, CLI, **Marquee-managed PCSX2 unpack**, **serial cover lookup**. `.iso` only; `.cue/.bin/.chd/.gz/.cso` are counted and skipped.

### Out of scope (do not add here)

ScreenScraper, SteamGridDB, Steam/Lutris, other emulators, BIN/CUE/CHD play, whole-ISO hashing/auto-relink, automated BIOS acquisition jobs (folder import exists today), per-game PCSX2 config, memory cards, updater, packaging, full `catalog/tools/` wizard, `gilrs`.

---

## How to run

### Prerequisites

- Fedora (or similar) with Rust, Node 22+, pnpm
- For the desktop window: `./scripts/setup.sh` (WebKit 4.1, DBus, and the rest of the [Tauri Fedora list](https://v2.tauri.app/start/prerequisites/))
- A folder of `.iso` files you already own
- PCSX2 (Install in the app downloads and unpacks it into Marquee), then drop your BIOS files or a zip into `<data dir>/bios-inbox`, or use Tools → BIOS import

```bash
cd apps/desktop
pnpm install
scripts/run.sh dev    # Vite + desktop shell
pnpm tauri dev        # real library (empty until you add a folder)
./scripts/run.sh desktop  # same, plus NVIDIA Wayland DMA-BUF workaround
```

Desktop starts empty and never falls back to sample games. “Return to Marquee” hides the overlay; it does not quit PCSX2. Settings → PlayStation 2 → Choose folder, then Play.

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps2 --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners list --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev health
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners install --channel auto
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners select runner.pcsx2.appimage
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev artwork refresh
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev launch <edition-id>
```

`launch` stays attached until the process exits. A second process on the same data dir fails with `INSTANCE_BUSY`. `--allow-fake` is a test runner (`/bin/sleep`), not an emulator.

| Path | Default |
| --- | --- |
| Library DB | `$XDG_DATA_HOME/marquee/library.db` |
| Unpacked PCSX2 | `$XDG_DATA_HOME/marquee/runners/pcsx2/current` (`usr/bin/pcsx2-qt`) |
| PCSX2 config / empty BIOS dir | `$XDG_DATA_HOME/marquee/pcsx2/xdg-config/PCSX2/` |
| Config | `$XDG_CONFIG_HOME/marquee/` |
| Cache | `$XDG_CACHE_HOME/marquee/` (covers under `art/`) |

Tests must use `--data-dir` / `Core::open_isolated`. Never point them at the real library. Tests must not discover a host PCSX2.

### Launch argv (structured; no shell)

```text
$XDG_DATA_HOME/marquee/runners/pcsx2/current/usr/bin/pcsx2-qt -batch -fullscreen -nogui -- /path/to/game.iso
(managed) runners/pcsx2/current/usr/bin/pcsx2-qt -batch -fullscreen -nogui -- /path/to/game.iso
```

### BIOS / setup status

| State | Meaning | Play |
| --- | --- | --- |
| missing | Configured BIOS location is readable and empty | Blocked; Open setup |
| unverified | PCSX2 present but not launch-proven, or BIOS path unresolved. Empty Marquee `…/pcsx2/xdg-config/PCSX2/bios` or default `~/.config/PCSX2/bios` is unverified, not missing | Allowed |
| launch-tested | Marquee supervised a successful boot of this install | Allowed |

Marquee never copies BIOS or rewrites PCSX2 settings.

### Errors

| Code | What to do |
| --- | --- |
| `ROOT_OFFLINE` | Folder missing or unmounted. Games stay listed. |
| `MEDIA_MISSING` | `.iso` moved or unsupported format. |
| `RUNNER_MISSING` | Install PCSX2 from Settings / the empty library, then Open setup for BIOS. |
| `SETUP_MISSING` | Open PCSX2 setup and add BIOS files. |
| `INSTALL_BUSY` | An install is already running. |
| `NETWORK` | GitHub or the cover host could not be reached, or a checksum failed. |
| `NOT_FOUND` | No cover for that serial (kept as skipped). |
| `LAUNCH_BUSY` | A game is already running. |
| `INSTANCE_BUSY` | Another Marquee process owns this data directory. |
| `SPAWN_FAILED` | Could not start PCSX2. Check the managed install and ISO path. |


---

## What landed (2026-10-20)

PS2 vertical slice implemented before the direct-store milestones. Metadata matching (ScreenScraper) was cut after review; serial JPEG covers stayed in this slice.

**On `origin/main`**

- Workspace: `crates/core`, `crates/cli`, `apps/desktop/src-tauri`.
- SQLite (rusqlite bundled, WAL, foreign keys, 5s busy timeout). Editions keep UUID, `scan_title` vs `display_title`, favourite.
- Recursive case-insensitive `.iso` scan, no symlink following, skipped formats counted.
- Bounded `SYSTEM.CNF` / `BOOT2` serial. Filename title if missing. Moved files = missing + new, never auto-merged.
- PCSX2 discovery (Marquee's unpacked tree only). One session, flock lock, spawn without `sh -c`, playtime on successful exit only.
- Tauri 2 + dialog + single-instance. Desktop vs explicit demo API. Empty library never sample-falls-back.
- `catalog/systems/ps2.json` built-in registry (no command fields).
- `scripts/setup.sh` for WebKit 4.1 and DBus devel.

**In the working tree (not pushed)**

- In-app PCSX2 install: **unpack** the official GitHub AppImage into `runners/pcsx2/current` (only channel). Provenance `marquee_installed_pcsx2`. Isolated `XDG_CONFIG_HOME` under `pcsx2/xdg-config/`.
- Install **dropdown → dialog** with one row (Recommended).
- Serial covers from xlenore/ps2-covers (`SLUS_209.63` → `SLUS-20963.jpg`), cached JPEG, HTTP outside the DB lock. 404 keeps procedural art.
- Schema v2 `artwork` table; reject a newer `user_version`; abandon stale `running` sessions on open (no playtime).
- Tests isolate host PCSX2 (`open_isolated` / `discover_host: false`).
- Window icon (`scripts/make-icon.py` + `pnpm tauri icon`).
- Current `scripts/run.sh desktop` launches production; the native startup policy selects accelerated shared-memory transport on NVIDIA/Wayland. `MARQUEE_RENDERER=software` retains the older `WEBKIT_DISABLE_DMABUF_RENDERER=1` fallback. Historical desktop observations below used that older software workaround.
- `scripts/check.sh`: missing WebKit still prints `BLOCKED` and continues; a failed `cargo check -p marquee-desktop` **exits 1**.
- Desktop UI matches the frontend chrome: same nav, ken-burns hero, cover rows; empty state uses procedural art + ghost shelves.

Host while building: Fedora Wayland, rustc 1.96.1, Node 24.18.0, pnpm 12.14.0. WebKit/DBus devel are installed.

### Definition of done

| Item | Status |
| --- | --- |
| UI in a Tauri window; browser demo explicit | **Pass** on this host (NVIDIA DMA-BUF workaround). Demo is explicit. Desktop chrome matches the frontend. Folder picker / Play not click-tested. |
| Desktop persistent backend; empty library | **Pass** on isolated `MARQUEE_DATA_DIR` (0 roots / 0 editions). Empty Home/Library keep console layout. |
| ISO import; skipped formats counted | **Done** in core/CLI/tests. Desktop picker wired (`dialog` plugin), not live-tested. |
| Rescan/restart keep IDs, favourites, titles | **Done** (automated). |
| One PCSX2 variant launched end-to-end | **Partly done:** managed PCSX2, Vice City (USA), exit 0, 254 s recorded (2026-10-21). Relaunch and restart-persistence still to confirm. |
| BIOS missing / unverified / launch-tested | **Done** in core. Open-setup not live-tested. |
| Supervised duration persisted once | Fake runner **yes**. Real PCSX2 **no**. |
| Reload / duplicate launch / interrupted sessions | **Done** in core: duplicate launch blocked; matching live processes are recovered; already-dead interrupted sessions retain checkpointed time. See [CORRECTION_PASS.md](CORRECTIONS.md). Desktop UI reload still live-pending. |
| PS2 collection + keyboard; controller in Tauri | Library filter + keyboard in demo/desktop. **Controller (`gilrs`) pending.** |
| Offline, no provider credentials | **Done.** Covers are optional; Play does not need them. |
| CLI same core, session ownership | **Done.** |
| Tests pass; desktop compile or blockers named | **Pass** on this host. 27 core tests; 16 UI tests; desktop `cargo check`. Live PCSX2 still pending. |

The slice is **implemented in tree**, **not release-complete**.

---

## Verification (2026-10-20)

Isolated data directories only. No commercial ISOs or BIOS in the repo.

| Item | Value |
| --- | --- |
| Host | Fedora Linux, Wayland (NVIDIA: DMA-BUF workaround required) |
| Primary PCSX2 variant | **Live Play observed 2026-10-21:** managed PCSX2 booted Vice City (USA) from the CLI; session ended `exit 0`, 254 s, stored once. The 254 s run was started by the agent and quit by the user; picture and input were not independently verified, and relaunch/restart checks remain. |
| Desktop compile | **Pass** (`cargo check -p marquee-desktop`) |
| Window | **Opens** via `./scripts/run.sh desktop`. Plain Wayland without the env dies with Gdk protocol error 71 on this NVIDIA host. |
| Empty UI chrome | **Pass** in the browser (`?splash=0&empty=1`): hero, ghost rows, Library grid, Ports empty banner. Native WebView of the same shell: process up; not re-screenshoted after the look change. |

`scripts/check.sh`: fmt; clippy/tests for core+CLI (excludes `marquee-desktop` from clippy/test); `cargo check -p marquee-desktop` prints `BLOCKED` if native deps are missing, **exits 1** if they are present and compile fails; then `pnpm check` / `pnpm test` / `pnpm build`.

| Check | Result |
| --- | --- |
| fmt, clippy core+CLI, core tests, CLI smoke | Pass (27 core tests: unpack AppImage, managed runner without network, cover skip, stale-session reclaim, isolated runners) |
| `pnpm check`, 16 UI tests, `pnpm build` | Pass |
| `cargo check -p marquee-desktop` | **Pass** (`protocol-asset` for cached covers) |
| `pnpm tauri dev` | **Process up** with `WEBKIT_DISABLE_DMABUF_RENDERER=1` |

Fake-runner tests do **not** close the real-emulator gate.

| # | Manual check | Result |
| --- | --- | --- |
| 1 | Fresh desktop, empty library, Add PS2 folder | Empty DB **yes**. UI chrome **yes** (browser). Native folder picker **pending**. |
| 2 | Import folder + skipped-format summary | Pending (CLI/tests cover synthetic ISOs) |
| 3–5 | Rescan IDs; restart; missing runner | Automated |
| 6–9 | Open setup; live launch; exit duration; relaunch | Pending |
| 10 | UI reload during fake session | Partial (core yes, desktop pending) |
| 11 | Missing file vs offline folder | Automated |
| 12 | Keyboard / controller in Tauri | Keyboard in demo; Tauri / `gilrs` pending |
| 13 | AppImage and Flatpak argv | **Removed** with host discovery |
| 14 | Install dropdown → dialog → click a channel | **Pass** in the browser demo Settings. Native install click pending. |
| 15 | Fetch covers after import | Automated skip/empty; live xlenore fetch pending |

| Variant | Discovery | Argv | Live launch |
| --- | --- | --- | --- |
| Unpacked `runners/pcsx2/current` | Yes (only variant) | `-batch -fullscreen -nogui -- <iso>` | **Booted and exited 0** (2026-10-21) |
| Host AppImage / `pcsx2-qt` / Flatpak | **Removed** (not discovered) | n/a | n/a |
| `--allow-fake` | Tests only | No PCSX2 flags | Automated; not an emulator gate |

---

## Next

**PS2 acceptance remains open.** Preserve this gate and the existing implementation while proceeding with the owner-authorized direct-store order: Steam, then Epic/Legendary with managed compatibility, then GOG/gogdl. Lutris is not required. [ROADMAP.md](ROADMAP.md) owns cross-project priority; this slice does not gate all other work.

1. Restart the desktop window (`./scripts/run.sh desktop` on this NVIDIA Wayland host) so the look, install, covers, and session reclaim are in the binary.
2. **Add a PS2 `.iso` folder** (native picker). Confirm no sample games, skipped-format summary if needed, covers after import.
3. If PCSX2 is missing, **Install → Recommended** (unpack into Marquee). Put your BIOS in `bios-inbox`; no setup window is needed.
4. **Play → quit emulator → duration once → selection restored.** Relaunch without stale busy. This is the release gate.
5. Keep `scripts/check.sh` green.

PS2 metadata follow-up: [PS2_METADATA_FOLLOWUP.md](ENRICHMENT.md#ps2-metadata-matching-and-cached-artwork). The next product increment is the [manager preparation foundation](MARQUEE_MANAGER.md#implementation-sequence-and-gates); [direct installed Steam import](STORE_INTEGRATION.md#s1a-installed-steam-games) may continue alongside it. Epic and GOG retain their order in the roadmap. Do not insert a Lutris milestone.

### On-demand PCSX2 install (built)

Marquee **owns** the PCSX2 tree, the same way `cx-script` owns `//DEPS` under `.cx/` instead of asking a distro wizard.

**Recommended** downloads the official `linux-appimage-x64-Qt.AppImage` from GitHub, checks the release sha256, and unpacks it with `--appimage-extract` into `$XDG_DATA_HOME/marquee/runners/pcsx2/current`. Play runs `usr/bin/pcsx2-qt` from that folder. No Flatpak, no FUSE, no host packages, no installer wizard. Config/BIOS for that copy live under `$XDG_DATA_HOME/marquee/pcsx2/xdg-config/PCSX2/` (Marquee creates an empty `bios/` dir; you drop files you already own).

Dialog row: **Recommended** (there is no Open-setup button any more). CLI: `marquee runners install [--channel auto|unpacked]`; other channels are rejected.

Never downloads BIOS or ISOs. Play uses `-batch -fullscreen -nogui` to hide PCSX2's main window while showing the game, then exit on game shutdown. This does not replace completing setup or supplying a BIOS. Open setup is the GUI for adding BIOS. Provenance is stored as `marquee_installed_pcsx2`.

### Box art (serial lookup)

After import, Marquee fetches 2D covers from [xlenore/ps2-covers](https://github.com/xlenore/ps2-covers) using the disc serial (`SLUS_209.63` → `SLUS-20963.jpg`), caches JPEG under the cache dir, and shows it on cards via the asset protocol. No ScreenScraper/SteamGridDB yet. Missing serial or 404 keeps procedural art. CLI: `marquee artwork refresh`. Settings → Fetch covers. Play still works offline with providers down.

---

## Code map

| Claim | Where |
| --- | --- |
| Schema v2 | `crates/core/src/db.rs` (`SCHEMA_VERSION`, `artwork`, abandon running sessions) |
| ISO scan + serial | `crates/core/src/library.rs`, `crates/core/src/iso.rs` (`cover_serial`) |
| PCSX2 argv + BIOS | `crates/core/src/runners.rs` |
| PCSX2 install | `crates/core/src/tools.rs` |
| HTTP (covers vs AppImage caps) | `crates/core/src/http.rs` |
| Covers | `crates/core/src/artwork.rs` |
| Sessions | `crates/core/src/sessions.rs` |
| Instance lock | `crates/core/src/instance.rs` |
| Isolated tests | `Core::open_isolated` in `crates/core/src/lib.rs` |
| Tests | `crates/core/src/tests.rs` (+ module tests in `iso`, `artwork`, `tools`, `catalog`) |
| CLI | `crates/cli/src/main.rs` |
| Command layer (window and web share it) | `crates/core/src/api.rs` (`dispatch`) |
| Tauri bridge | `apps/desktop/src-tauri/src/commands.rs` (`core_call`, `web_url`), `lib.rs` (starts the server, forwards events) |
| Event stream | `crates/core/src/hub.rs` |
| API keys (registry, precedence, masking) | `crates/core/src/keys.rs`; Settings → API keys in `Settings.svelte` |
| Local web server (token, loopback) | `crates/core/src/server.rs`, `marquee run web` |
| Native ports | `crates/core/src/ports.rs`, `catalog/ports/*.json`, `apps/desktop/src/lib/screens/Ports.svelte` |
| Native vs web | `apps/desktop/src/lib/api/` (`desktop.ts`, `http.ts`, shared `core.ts`) |
| Play / empty library | `apps/desktop/src/lib/state.svelte.ts` |
| Install UI | `apps/desktop/src/lib/install.ts`, `InstallMenu.svelte`, `InstallDialog.svelte` |
| Empty shelves | `apps/desktop/src/lib/components/GhostCover.svelte` |
| Nav | `apps/desktop/src/App.svelte` (Home / Library / Ports / Downloads / Settings; `ps2` route still works) |
| Registry | `catalog/systems/ps2.json` |
| NVIDIA Wayland | `scripts/run.sh desktop` |
