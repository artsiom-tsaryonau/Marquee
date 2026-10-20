# PlayStation 2 (offline slice)

Last updated: 2026-10-20.

This is the only document for the current milestone: **what exists**, **how to run it**, **what was verified**, and **what is next**. Long-term phases: [ROADMAP.md](ROADMAP.md). Other systems: [EXPANSION_CATALOGUE.md](EXPANSION_CATALOGUE.md). Next ticket (not started): [PS2_METADATA_FOLLOWUP.md](PS2_METADATA_FOLLOWUP.md).

Release boundary:

> A persistent, offline PS2 library that launches through an existing PCSX2 installation, observes its exit, and records actual session duration.

That gate is **not closed**. Code is in the tree; a real PCSX2 launch/exit has not been observed on the implementation host. Do not start Steam/Lutris or metadata work until it is, unless the owner reorders.

Marquee does not download BIOS files or commercial games.

---

## Current state

| Surface | State |
| --- | --- |
| Browser demo (`pnpm dev`) | Works. Sample games, simulated launch, fictional +42 min. Ports / Downloads / Add Windows app are demo-only. |
| CLI (`marquee`) | Works without GTK. Same core as desktop. Use `--data-dir` for isolation. |
| Desktop (`pnpm tauri dev`) | **Code present, not compiled here.** Missing `webkit2gtk4.1-devel` and `dbus-devel`. |
| Persistent library | SQLite at `$XDG_DATA_HOME/marquee/library.db` (or `--data-dir` / `MARQUEE_DATA_DIR`). |
| Real PCSX2 Play | **Pending.** No AppImage / `pcsx2-qt` / Flatpak `net.pcsx2.PCSX2` on this host. |
| Controller in Tauri WebView | **Pending.** Gamepad API only; `gilrs` not added. Browser gamepad is not evidence. |
| Metadata / downloaded art | **Not started.** Filename titles + procedural art. |

Working tree: uncommitted UI audit tests (14) plus this Cargo workspace. Nothing was committed unless you commit it later.

### In scope for this slice

ISO import, PCSX2 discovery/argv, one supervised session, real playtime, offline UI, CLI. `.iso` only; `.cue/.bin/.chd/.gz/.cso` are counted and skipped.

### Out of scope (do not add here)

ScreenScraper, SteamGridDB, Steam/Lutris, other emulators, BIN/CUE/CHD play, whole-ISO hashing/auto-relink, BIOS file management, per-game PCSX2 config, memory cards, updater, packaging. On-demand PCSX2 install is **investigated, not built** (below).

---

## How to run

### Prerequisites

- Fedora (or similar) with Rust, Node 22+, pnpm
- For the desktop window: [Tauri Linux packages](https://v2.tauri.app/start/prerequisites/), including `webkit2gtk4.1-devel` and `dbus-devel`
- A folder of `.iso` files you already own
- PCSX2 as AppImage or Flatpak (`net.pcsx2.PCSX2`), with BIOS already configured inside PCSX2

```bash
cd apps/desktop
pnpm install
pnpm dev          # browser demo, sample data
pnpm tauri dev    # real library (empty until you add a folder)
```

Desktop starts empty and never falls back to sample games. Settings → PlayStation 2 → Choose folder, then Play. “Return to Marquee” hides the overlay; it does not quit PCSX2.

```bash
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps2 ~/Games/ps2
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps2 --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners list --json
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev runners select runner.pcsx2.appimage
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev launch <edition-id>
```

`launch` stays attached until the process exits. A second process on the same data dir fails with `INSTANCE_BUSY`. `--allow-fake` is a test runner (`/bin/sleep`), not an emulator.

| Path | Default |
| --- | --- |
| Library DB | `$XDG_DATA_HOME/marquee/library.db` |
| Config | `$XDG_CONFIG_HOME/marquee/` |
| Cache | `$XDG_CACHE_HOME/marquee/` |

Tests must use `--data-dir`. Never point them at the real library.

### Launch argv (structured; no shell)

```text
<pcsx2> -batch -fullscreen -- /path/to/game.iso
flatpak run net.pcsx2.PCSX2 -batch -fullscreen -- /path/to/game.iso
```

### BIOS / setup status

| State | Meaning | Play |
| --- | --- | --- |
| missing | Configured BIOS location is readable and empty | Blocked; Open setup |
| unverified | PCSX2 present but not launch-proven, or BIOS path unresolved. Empty default `~/.config/PCSX2/bios` is unverified, not missing | Allowed |
| launch-tested | Marquee supervised a successful boot of this install | Allowed |

Marquee never copies BIOS or rewrites PCSX2 settings.

### Errors

| Code | What to do |
| --- | --- |
| `ROOT_OFFLINE` | Folder missing or unmounted. Games stay listed. |
| `MEDIA_MISSING` | `.iso` moved or unsupported format. |
| `RUNNER_MISSING` | Install PCSX2 and restart Marquee. |
| `SETUP_MISSING` | Open PCSX2 setup and add BIOS files. |
| `LAUNCH_BUSY` | A game is already running. |
| `INSTANCE_BUSY` | Another Marquee process owns this data directory. |
| `SPAWN_FAILED` | Could not start PCSX2. Check the executable/Flatpak and ISO path. |

Flatpak may hide the library folder from PCSX2. Grant that folder, not the whole filesystem.

---

## What landed (2026-10-20)

PS2 vertical slice ahead of roadmap phase 1. Metadata was cut after review.

- Workspace: `crates/core`, `crates/cli`, `apps/desktop/src-tauri`.
- SQLite (rusqlite bundled, WAL, foreign keys, 5s busy timeout). Editions keep UUID, `scan_title` vs `display_title`, favourite.
- Recursive case-insensitive `.iso` scan, no symlink following, skipped formats counted.
- Bounded `SYSTEM.CNF` / `BOOT2` serial. Filename title if missing. Moved files = missing + new, never auto-merged.
- PCSX2 discovery (AppImage / `pcsx2-qt` / Flatpak). One session, flock lock, spawn without `sh -c`, playtime on successful exit only.
- Tauri 2 + dialog + single-instance. Desktop vs explicit demo API. Nav: Home / Library / PS2 / Settings.
- `catalog/systems/ps2.json` built-in registry (no command fields).

Host while building: Fedora Wayland, rustc 1.96.1, Node 24.18.0, pnpm 12.14.0. `sudo dnf install` of Tauri packages failed (password required).

### Definition of done

| Item | Status |
| --- | --- |
| UI in a Tauri window; browser demo explicit | **Partial.** Demo explicit. Window not compiled here. |
| Desktop persistent backend; empty library | **Code done.** Native window pending. |
| ISO import; skipped formats counted | **Done** in core/CLI/tests. Desktop picker not live-tested. |
| Rescan/restart keep IDs, favourites, titles | **Done** (automated). |
| One PCSX2 variant launched end-to-end | **Pending.** Discovery + argv implemented. |
| BIOS missing / unverified / launch-tested | **Done** in core. Open-setup not live-tested. |
| Supervised duration persisted once | Fake runner **yes**. Real PCSX2 **no**. |
| Reload / duplicate launch / interrupted sessions | Duplicate + fake end tested. Desktop reload pending. |
| PS2 collection + keyboard; controller in Tauri | Collection + keyboard in demo. **Controller pending.** |
| Offline, no provider credentials | **Done.** |
| CLI same core, session ownership | **Done.** |
| Tests pass; desktop compile or blockers named | Core/UI pass. Blockers: WebKit/DBus devel, no PCSX2. |

The slice is **implemented in tree**, **not release-complete**.

---

## Verification (2026-10-20)

Isolated data directories only. No commercial ISOs or BIOS in the repo.

| Item | Value |
| --- | --- |
| Host | Fedora Linux, Wayland |
| Primary PCSX2 variant | **Pending** |
| Desktop compile | **Pending** — `webkit2gtk4.1-devel`, `dbus-devel` |

`scripts/check.sh`: fmt; clippy/tests for core+CLI (excludes `marquee-desktop`); `cargo check -p marquee-desktop` prints `BLOCKED` if native deps are missing; then `pnpm check` / `pnpm test` / `pnpm build`.

| Check | Result |
| --- | --- |
| fmt, clippy core+CLI, 16 core tests, CLI smoke | Pass |
| `pnpm check`, 14 UI tests, `pnpm build` | Pass |
| `cargo check -p marquee-desktop` / `pnpm tauri dev` | **Blocked** |

Fake-runner tests do **not** close the real-emulator gate.

| # | Manual check | Result |
| --- | --- | --- |
| 1 | Fresh desktop, empty library, Add PS2 folder | Pending |
| 2 | Import folder + skipped-format summary | Pending (CLI/tests cover synthetic ISOs) |
| 3–5 | Rescan IDs; restart; missing runner | Automated |
| 6–9 | Open setup; live launch; exit duration; relaunch | Pending |
| 10 | UI reload during fake session | Partial (core yes, desktop pending) |
| 11 | Missing file vs offline folder | Automated |
| 12 | Keyboard / controller in Tauri | Keyboard in demo; Tauri pending |
| 13 | AppImage and Flatpak argv | Automated; live launch pending |

| Variant | Discovery | Argv | Live launch |
| --- | --- | --- | --- |
| AppImage / `pcsx2-qt` | Yes | `-batch -fullscreen -- <iso>` | Pending |
| Flatpak `net.pcsx2.PCSX2` | Yes | Same flags after `flatpak run …` | Pending |
| `--allow-fake` | Tests only | No PCSX2 flags | Automated; not an emulator gate |

---

## Next

1. Install `webkit2gtk4.1-devel`, `dbus-devel`, and remaining Tauri Fedora deps. `cargo check -p marquee-desktop` then `pnpm tauri dev`.
2. Empty library + Add PS2 folder, no sample fallback.
3. Install one PCSX2 by hand (Flathub user Flatpak preferred; official GitHub AppImage fallback). Record it as the live-tested primary. Marquee does not install it yet.
4. Import a user-owned `.iso` folder. Confirm skipped-format summary if other extensions are present.
5. Open PCSX2 setup (not playtime). Confirm BIOS state is not a false “missing” block.
6. Play → quit emulator → duration once → selection restored. Relaunch without stale busy state.
7. Controller in the **Tauri** window. Add `gilrs` only if Gamepad API fails. Keyboard-only is a limited preview until then.
8. Update the pending rows above. Do not download BIOS or games to manufacture a pass.

Then: [PS2_METADATA_FOLLOWUP.md](PS2_METADATA_FOLLOWUP.md). After that, roadmap phase 1 (Lutris/Steam).

### On-demand PCSX2 install (investigated, not built)

Marquee can offer “Want PS2? Install PCSX2”. Discovery already covers AppImage, `pcsx2-qt`, and Flatpak `net.pcsx2.PCSX2`. There is no install path yet.

| Channel | Status on this host | Role |
| --- | --- | --- |
| Flathub `flatpak install --user --noninteractive flathub net.pcsx2.PCSX2` | Flatpak 1.18.4 + Flathub remote present; app v2.8.2, 43.5 MB + KDE Platform 6.10 | **Default** — no root, matches discovery id, `host:ro` so ISO folders are readable |
| Official GitHub AppImage `pcsx2-v*-linux-appimage-x64-Qt.AppImage` | fuse3 present; asset has sha256 | **Fallback** — drop under `~/.local/share/marquee/runners/` or `~/Applications`, chmod +x, verify digest |
| Fedora RPM `pcsx2` | No package | Skip |
| GitHub `.flatpak` bundle | Exists on the v2.8.2 release | Skip — duplicates Flathub without remote updates |

Never download BIOS or ISOs. After install, rediscover runners, then **Open setup** for BIOS (default Flatpak BIOS dir is inside the sandbox; a custom BIOS dir needs a write override because PCSX2 writes `.nvm`). Track Marquee-installed vs user-installed so Remove cannot delete a copy the user already had.

First implementation, if asked: PCSX2 only, Flathub then AppImage, offer on `RUNNER_MISSING` / Settings. Do not build the full `catalog/tools/` catalogue to close this. Roadmap “Launchers and tools on demand” stays the later generalization.

---

## Code map

| Claim | Where |
| --- | --- |
| Schema | `crates/core/src/db.rs` |
| ISO scan + serial | `crates/core/src/library.rs`, `crates/core/src/iso.rs` |
| PCSX2 argv + BIOS | `crates/core/src/runners.rs` |
| Sessions | `crates/core/src/sessions.rs` |
| Instance lock | `crates/core/src/instance.rs` |
| Tests | `crates/core/src/tests.rs` |
| CLI | `crates/cli/src/main.rs` |
| Tauri commands | `apps/desktop/src-tauri/src/commands.rs` |
| Desktop vs demo | `apps/desktop/src/lib/api/` |
| Play / empty library | `apps/desktop/src/lib/state.svelte.ts` |
| PS2 collection | `apps/desktop/src/App.svelte` (`ps2` route) |
| Registry | `catalog/systems/ps2.json` |
