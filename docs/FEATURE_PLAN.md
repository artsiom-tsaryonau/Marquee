# Feature plan

Created 2026-11-25 from a four-pass read of `main` at `9453414`, after the
[hardening plan](HARDENING_PLAN.md) landed. This document splits the next
product features into agent-sized change sets with file anchors, tests,
acceptance and decisions. It does not reorder the [roadmap](ROADMAP.md) and
does not replace the owning guides: where a guide already has a task list
(stores, portable packages, streaming gates), this plan links to it and adds
only the code anchors, slicing and order. No item here closes a live gate.

Line numbers are as of `9453414`; re-locate by symbol name if they moved.

## Rules for executing agents

- Take one item per change set. Items that share files are serialized in the
  [conflict map](#conflict-map).
- Read the files and the owning guide named in the item first. Re-verify
  "Current state"; if it is no longer true, stop and record that here.
- Preserve existing IDs, data paths, receipts, saves, settings, tokens,
  personal titles and sessions. Never reset another contributor's changes.
- Schema: claim the next number from the [schema allocation](#schema-allocation)
  table in the same change set. Additive tables follow the existing
  `ensure_table` pattern (`crates/core/src/db.rs:181-182`) and do not bump the
  version.
- Pins: every new downloaded executable is pinned (tag, asset URL, SHA-256,
  binary SHA-256 where the asset is an archive) and fetched through
  `marquee_media::http::download_verified`. Never add an unpinned recipe.
- Do not download, copy or commit game data, BIOS, firmware or keys. Tests use
  synthetic fixtures.
- There is no hosted CI and none should be added: do not create
  `.github/workflows/`. Run `scripts/check.sh` before handing back. A `BLOCKED`
  line for missing WebKit/DBus devel packages is acceptable only if stated.
- Do not commit or push unless the owner asks.
- Live acceptance (a real game booting, input, saves, a second device) is the
  owner's to perform. Unit tests never mark live behavior verified. Record a
  manual check you could not perform as unperformed.
- When an item is done, update its status in the [tracker](#tracker) and add a
  dated line in the [implementation log](#implementation-log), plus the result
  in the owning guide.

## Verified baseline (2026-11-25)

| Area | Fact | Where |
| --- | --- | --- |
| Sessions | `sessions.edition_id` is `NOT NULL REFERENCES editions(id)`; `runner_id` references `runners`. Sessions record start/end/duration/exit/state plus `process_identity` (v3). `complete_session` credits `editions.playtime_seconds`, `play_count`, `last_played_at`. Exits under 10 s are failures | `crates/core/src/db.rs:83-93`, `:221`; `crates/core/src/sessions.rs:43`, `:253`, `:339` |
| Port launch | `Core::launch_port` → `port_engine::launch` spawns the binary, discards stderr and reaps on a thread. No session row. Only Shipwright gets env (`SHIP_HOME`, `APPIMAGE_EXTRACT_AND_RUN`) | `crates/core/src/lib.rs:610`; `crates/manager/src/port_engine.rs:716-753` |
| Port data | Each port's game data (ROM, WAD, Fallout data) is dropped manually into `<data>/ports/<id>/`. Backend status `ready` means the engine is installed, not that data is present. UI has `needs-rom`/`needs-data` labels that the backend never sends | `port_engine.rs:105-120`; `apps/desktop/src/lib/presentation.ts` |
| Port pins | Six recipes compiled in with `include_str!`; `pinned_release.{tag, asset, sha256, binary_sha256}`. Pinned install whose upstream tag differs reports `review`. No runtime override | `port_engine.rs:19-56`, `:109-120` |
| Port playtime UI | `nativePortGame` sets `playtimeMin: 0`; Game page shows "Not tracked" for ports | `apps/desktop/src/lib/unified.ts:43-49`; `GamePage.svelte:~160` |
| Play adapters | A system gets Play from `catalog/systems/<id>.json` `emulator`, plus a catalogue emulator with `kind` and `launch`; `emulators::sync_runners` registers `runner.<id>.marquee`; `runners::launch_argv` appends `{rom}` | `crates/core/src/emulators.rs:~1003`; `crates/core/src/runners.rs:~194-229` |
| PS4/PS5 | shadPS4 has `kind: emulator` and `-g` launch; no `catalog/systems/ps4.json` yet. `kytyps5.json` has no `kind`. Read-only `console::inspect` exists. ShadPKG base extraction of CUSA08809 passed only as an isolated operator spike | `catalog/emulators/shadps4.json`; `crates/core/src/console.rs`; [console preparation](CONSOLE_PREPARATION.md) |
| Xbox 360 | Verified ISO prepare/register/Play with Xenia; storage/content/cache/config/log forced under `<data>/xenia/`. Vanquish menu, audio, keyboard, clean exit observed; gameplay and native save not | `emulators.rs:~789-833`; [Xbox 360 adapter](CONSOLE_PREPARATION.md#xbox-360-verified-iso-adapter-2026-10-25) |
| Streaming API | Streaming screen covers an existing host: connection, viewer install/pair/open/disconnect, pairing, revoke, end game, and Wolf runtime build/probe/rollback. No host Start button | `crates/core/src/streaming/controls.rs`; `runtime.rs`; `apps/desktop/src/lib/screens/Streaming.svelte` |
| Headless | Allowlist is `(genesis, runner.blastem.marquee)` and `(xbox360, runner.xenia.marquee)`. Genesis uses a reviewed Sonic/BlastEm image or composes one from managed Wolf and BlastEm. Xbox 360 still requires the pinned Xenia receipt. Ports publish on `127.0.0.1` only | `crates/core/src/streaming/headless.rs`; `catalog/streaming/library/library-game.py` |
| Stream playtime | Schema v7 `sessions.mode`. Headless inserts `mode='stream'` when the emulator holds its profile lock, checkpoints while it runs, and completes when it exits. Viewer close does not end playtime | `streaming/headless.rs`; `db.rs` v7 |
| Viewer target | Moonlight `stream`/`pair` target is the literal `127.0.0.1`; Wolf `WOLF_INTERNAL_IP` is `127.0.0.1` | `streaming/viewer.rs:70-72`, `:120-123`; `catalog/streaming/wolf/entrypoint.py:57` |
| Steam | `steam::discover()` builds a transient `Inventory` (`steam-install:{lib_fp}:{appid}`); exposed as `inspect_steam` (API/CLI), not persisted, not in the desktop UI, no launch. `steam_web_api`/`steam_id` keys exist with no consumer | `crates/core/src/steam/mod.rs:145-225`; `lib.rs:644`; `crates/core/src/keys.rs:41-57` |
| Windows runtime | Only Purple: pinned umu zipapp, isolated `<data>/windows/purple/`, receipt, cancel. No Legendary/gogdl code | `crates/core/src/purple.rs:21-89` |
| Portable packages | Verified baseline above is 2026-11-25. Since 2026-11-28 `.marquee` export/import works from the CLI (MP1) | [portable packages](PORTABLE_PACKAGES.md); `crates/core/src/packages.rs` |
| Storage | Inventory, configurable collection/BIOS folders, same-filesystem managed installation moves with journal and rollback. No cross-filesystem move, no cleanup candidates, no artifact registry | `crates/core/src/storage.rs`; `crates/manager/src/relocation.rs`; `crates/core/src/storage_moves.rs` |
| Identity | Schema v7. `editions` is effectively an installation; no `games`/`installations` tables | `crates/core/src/model.rs:4`; [unified library](UNIFIED_LIBRARY.md) |
| Soundtracks | Pinned yt-dlp used for metadata only (`--skip-download`, cookies/plugins/remote components off); local `music_import` copies into content-addressed storage. No manager music job | `crates/core/src/music_discovery.rs:16-237`; `crates/core/src/music.rs:449` |
| Controllers | No `gilrs`; browser Gamepad API drives focus only; keyboard seeds for Xenia, PCSX2, Mesen | `apps/desktop/src/lib/focus.ts`; `crates/core/src/bios.rs:336-396`; `catalog/emulators/xenia.json` |
| Saves | Emulator profiles are per emulator (`<data>/<emulator>/xdg-*`), not per game. Ports keep saves in `<data>/ports/<id>/`. Port engine updates keep `.engine-backup`. No save backup feature | `emulators.rs:~789-833`; `port_engine.rs:~450-522` |
| Tests | Rust unit tests in-module plus `crates/*/tests/`; desktop tests are `apps/desktop/tests/*.test.mjs` run by `pnpm test` through `tests/register.mjs`; script tests under `scripts/tests` | `scripts/check.sh`; `apps/desktop/package.json` |

## Owner decisions

Agents must not decide these. The recommended default applies only when the
owner says to proceed with defaults. The owner said to follow the plan, so the
recommended defaults below are accepted.

| ID | Question | Decision | Blocks |
| --- | --- | --- | --- |
| FD1 | Where do port play sessions live? | Accepted: new additive `port_sessions` table; leave `sessions` and its foreign keys untouched | NP1 |
| FD2 | Does streamed play count toward a game's playtime? | Accepted: yes. Record a normal session with `mode = 'stream'` and show the split on the Game page | ST3c |
| FD3 | Re-pinning a port: maintainer-only, or a user-local pin override? | Accepted: maintainer-only. `scripts/repin-port.py` writes the pin; there is no runtime override | NP5 |
| FD4 | Save snapshot scope | Accepted: whole emulator profile data directory and whole port directory (minus engine files), on demand and before emulator/port updates; keep the newest 5 per scope | F2 |
| FD5 | Soundtrack download format | Accepted: best audio stream as published (no transcode, no FFmpeg dependency); only URLs the owner linked | F3 |
| FD6 | LAN streaming exposure | Accepted: off by default. When enabled, publish on one owner-selected interface address, never `0.0.0.0` | ST5 |
| ST6 | Play on a phone: Moonlight for iOS/Android with its on-screen touch controller, per-system layouts | Streaming | M | ST5, HL2 | Open (owner ask 2026-11-27). Moonlight's mobile clients already draw a virtual gamepad, so no overlay of Marquee's own is needed first; needs LAN publish (ST5) and headless adapters for each runner (HL1/HL2, then Cemu, xemu, PPSSPP, Azahar, Ymir) |
| QA1 | Play checks: Library Play inside a private headless gamescope with screenshots and an optional recording | Emulators | S | — | Done 2026-11-28: `scripts/play-check.sh <edition> [--seconds] [--shots] [--record]`; `MARQUEE_LAUNCH_PREFIX` wraps the game command and `MARQUEE_DISPLAY_MODE=window` forces a window, so the check runs Marquee's real Play path. Screenshots via `gamescopectl screenshot`; recording from gamescope's PipeWire node (by serial) through GStreamer openh264 to MP4. Nothing appears on the desktop or speakers: the game alone (after gamescope's `--`) gets no audio server (SDL dummy driver, OpenAL null, `PULSE_SERVER`/`PIPEWIRE_REMOTE` pointing at missing sockets), and `scripts/play_check_audio_guard.py` kills the game if any stream still reaches an output. A private null sink was not enough: shadPS4's SDL opens the default device by name and bypassed it twice. 2026-11-28 screenshots: The Last Guardian title, Metroid Prime title, Smash intro, Forza intro, Persona 3 Portable opening, Nightshade title (60 fps), Breath of the Wild opening (30 fps), The King of Fighters XV title (CUSA25273, not on shadPS4's compatibility list). In window mode Cemu and Eden show their menu and status bars |
| REC1 | Native recording and screenshots in Marquee | Emulators | M | QA1 | Open (owner ask 2026-11-28). Play optionally runs the game in a nested gamescope window on the desktop (Wayland backend, not headless); a Record button and screenshot key capture its PipeWire stream to the game's media (NVENC through FFmpeg or GStreamer when available). The same stream can feed streaming (HL1/ST5) |
| FD7 | Embedded player | Accepted: research only. The decision is recorded under ST4 in the streaming guide; do not build a receiver | ST4 |
| FD8 | Native controller library | Accepted: `gilrs` in the Tauri process for inventory and UI navigation; emulator binding profiles only after hardware arrives | F4 |
| FD9 | Epic/GOG backends | Accepted: pin upstream Legendary and heroic-gogdl Linux binaries when their release assets verify; otherwise pin a source commit built in the existing rootless source-build path | E1, G1 |

## Schema allocation

| Version | Item | Change |
| --- | --- | --- |
| 6 | (shipped) | `preparation_receipts` |
| 7 | ST3c | `sessions.mode TEXT NOT NULL DEFAULT 'local'` |
| 8 | UL1 | Normalized game/edition/installation tables ([unified library](UNIFIED_LIBRARY.md)) |
| additive | NP1, S1A, F1, F2, F3 | Tables via `ensure_table`, no version bump |

If UL1 lands before ST3c, swap the numbers here in the same change set.

## Tracker

| ID | Item | Track | Size | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| NP1 | Port play sessions and playtime | Ports | M | FD1 | Closed 2026-11-26. Implementation and live Shipwright play accepted |
| NP2 | Port data readiness (`needs-rom` / `needs-data`) | Ports | S | — | Done 2026-11-25 |
| NP3 | Shipwright game-data setup job | Ports | M | NP2 | Closed 2026-11-26. Live ROM setup, native save, relaunch and restore accepted |
| NP4 | Data setup for the other five ports, one per change set | Ports | S each | NP2 | 2ship2harkinian and sm64coopdx done 2026-11-26 (live ROM setup needs an owner dump). Zelda64Recomp, Fallout 2, UZDoom open |
| NP5 | Port release review screen and re-pin script | Ports | S | FD3 | Done 2026-11-25 |
| PS4-1 | shadPS4 launch contract and runner | Consoles | S | — | Done 2026-11-26. Live title boot unperformed |
| PS4-2 | PS4 base-game preparation recipe | Consoles | L | PS4-1 | Base done by hand 2026-11-28: The Last Guardian EU (CUSA03745 01.00) RAR → PKG → ShadPKG → 52 files, 25 GB, in `library/imported-roms/ps4/<title> [ID]/`. Updates need the patched LibOrbisPkg. Open: the same steps as an intake job |
| PS4-3 | PS4 system descriptor and Library Play | Consoles | M | PS4-2 | Done 2026-11-28: `ps4` system, the `eboot.bin` beside `sce_sys/param.sfo` is the entry and names the game. shadPS4 0.21.0 installed. The Last Guardian (title screen) and The King of Fighters XV (title screen, 90 s stable) run from Library Play. Per-title fixes ship as seeded `custom_configs/<ID>.json`: CUSA03745 gets `readbacks_mode: 2` (precise), the setting the compatibility list needs for correct graphics. The very first shadPS4 start died on a signal at 0 s; later starts work |