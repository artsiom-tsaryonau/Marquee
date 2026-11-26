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
| Portable packages | Proposal only; manager `Receipt` has `id, recipe, source_sha256, system, destination, files[path,size,sha256]` | [portable packages](PORTABLE_PACKAGES.md); `crates/manager/src/service.rs:35-49` |
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
| NP1 | Port play sessions and playtime | Ports | M | FD1 | Done 2026-11-26. Live Shipwright play passed 2026-11-26 |
| NP2 | Port data readiness (`needs-rom` / `needs-data`) | Ports | S | — | Done 2026-11-25 |
| NP3 | Shipwright game-data setup job | Ports | M | NP2 | Done 2026-11-26. Live ROM setup, save and restore passed 2026-11-26 |
| NP4 | Data setup for the other five ports, one per change set | Ports | S each | NP2 | 2ship2harkinian done 2026-11-26. Zelda64Recomp, sm64coopdx, Fallout 2, UZDoom open |
| NP5 | Port release review screen and re-pin script | Ports | S | FD3 | Done 2026-11-25 |
| PS4-1 | shadPS4 launch contract and runner | Consoles | S | — | Done 2026-11-26. Live title boot unperformed |
| PS4-2 | PS4 base-game preparation recipe | Consoles | L | PS4-1 | Open |
| PS4-3 | PS4 system descriptor and Library Play | Consoles | M | PS4-2 | Open |
| PS5-1 | KytyPS5 pin and launch contract | Consoles | S | — | Done 2026-11-26. Live --help and save-path check unperformed |
| PS5-2 | PS5 preparation and Library Play | Consoles | L | PS5-1 | Open |
| X360-1 | Xbox 360 save/relaunch acceptance support | Consoles | S | — | Done in code. Live Vanquish save/relaunch unperformed |
| ST3a | Wolf runtime build/status/probe/rollback in the app | Streaming | M | — | Done 2026-11-26. No host Start button |
| ST3b | Base image independent of the Xenia pilot | Streaming | M | — | Done 2026-11-26. Live Genesis stream unperformed |
| ST3c | Streamed playtime | Streaming | M | FD2 | Done 2026-11-26. Live stream playtime unperformed |
| HL1 | Mesen headless adapter | Streaming | M | ST3b | Open |
| HL2 | DuckStation and PCSX2 headless adapters | Streaming | M each | HL1 | Open |
| ST4 | Embedded receiver research spike | Streaming | S | FD7 | Done 2026-11-25. No receiver code |
| ST5 | Configurable stream target and opt-in LAN publish | Streaming | M | FD6 | Open |
| S1A | Installed Steam library: persist, show, launch | Stores | L | — | Open |
| S1B | Owned Steam library sync | Stores | L | S1A | Open |
| C1 | Shared managed Windows runtime (umu/Proton) | Stores | L | — | Open |
| E1 | Epic through Legendary | Stores | XL | C1, FD9 | Open |
| G1 | GOG through gogdl | Stores | XL | C1, FD9 | Open |
| MP0 | Portable package manifest and fixture | Collection | S | — | Done 2026-11-25 |
| MP1 | Local export and import | Collection | L | MP0 | Open |
| MP2 | Cross-instance acceptance | Collection | M | MP1 | Open |
| MP3 | Private transfer | Collection | L | MP2 | Open |
| SL1 | Reviewed source cleanup candidates | Collection | M | — | Open |
| SL2 | Cross-filesystem installation moves | Collection | M | — | Open |
| UL1 | Normalized identity model | Collection | XL | — | Open |
| F1 | Playtime history and recently played | New | S | NP1 (optional) | Done 2026-11-26, including port sessions |
| F2 | Save snapshots and restore | New | M | FD4 | Done 2026-11-26 |
| F3 | Soundtrack downloads (MU4) | New | L | FD5 | Open |
| F4 | Controller inventory and native navigation | New | M | FD8 | Open |

## Waves

Items in one wave can run in parallel unless the conflict map says otherwise.

1. **Wave 1, small and independent:** NP2, NP5, PS4-1, PS5-1, X360-1, ST3b,
   ST4, MP0, F1.
2. **Wave 2, foundations:** NP1, ST3a, ST3c, S1A, C1, SL1, F2.
3. **Wave 3, built on wave 2:** NP3, NP4, PS4-2, HL1, MP1, SL2, F3, F4.
4. **Wave 4:** PS4-3, PS5-2, HL2, ST5, S1B, E1, MP2.
5. **Wave 5:** G1, MP3, UL1.

UL1 can start earlier as a design and migration-fixture change, but its cutover
waits until S1A and E1 show which installation shapes the model must hold.

## Conflict map

| File | Items |
| --- | --- |
| `crates/core/src/db.rs` | NP1, ST3c, S1A, F1, F2, F3, UL1 — serialize; follow the schema allocation |
| `crates/core/src/sessions.rs` | NP1, ST3c, X360-1 — serialize, NP1 first |
| `crates/manager/src/port_engine.rs` | NP2, NP3, NP4, NP5, F2 — serialize in that order |
| `crates/core/src/lib.rs` | NP1, NP2, S1A, F2 — small edits, rebase carefully |
| `crates/core/src/api.rs` | Most items — one command block per item |
| `crates/core/src/emulators.rs` | PS4-1, PS5-1, F2 |
| `crates/core/src/streaming/headless.rs` | ST3b, ST3c, HL1, HL2, ST5 — serialize, ST3b first |
| `catalog/streaming/library/*` | ST3b, HL1, HL2 |
| `crates/core/src/streaming/viewer.rs`, `controls.rs` | ST3a, ST5 |
| `crates/manager/src/service.rs`, `daemon.rs` | NP3, PS4-2, PS5-2, MP1, F3 |
| `apps/desktop/src/lib/state.svelte.ts` | Most UI items — keep each item's additions in one block |
| `apps/desktop/src/lib/screens/GamePage.svelte` | NP1, ST3c, F1, F2 |
| `apps/desktop/src/lib/screens/Home.svelte` | F1 |
| `apps/desktop/src/lib/presentation.ts` | NP2, NP5 |

---

## NP1 — Port play sessions and playtime

**Owner guide:** [Quiver Q2](QUIVER_INTEGRATION.md), [corrections 2026-10-20](CORRECTIONS.md).

**Why.** Ports are the owner's first priority and show "Not tracked". The Q2
gate requires launch, return and relaunch, which needs a recorded session.

**Current state.** `port_engine::launch` (`port_engine.rs:716`) spawns and
forgets. `sessions` cannot hold a port row because `edition_id` must exist in
`editions` (`db.rs:85`).

**Change.**
1. Add `crates/core/src/port_sessions.rs` with `ensure_table` creating
   `port_sessions(id TEXT PRIMARY KEY, port_id TEXT NOT NULL, started_at TEXT
   NOT NULL, ended_at TEXT, duration_seconds INTEGER, exit_code INTEGER, state
   TEXT NOT NULL, process_identity TEXT, error TEXT)` and an index on
   `port_id, started_at`. Call it beside `romfacts`/`wanted` in `db.rs:181`.
2. Move the spawn out of `port_engine::launch` into a function that returns the
   `Child` (keep `configure_launch` in the manager crate). In core, reuse the
   session lifecycle from `sessions.rs`: `ProcessIdentity` binding, 250 ms
   checkpoints, the 10 s early-exit rule, `recover` on startup. Factor the
   shared wait/checkpoint loop rather than copy it.
3. Keep one running game at a time: `launch_port` must refuse while
   `db.running_session()` or a running port session exists, and `Core::launch`
   must refuse while a port session runs. Headless refuses both.
4. Emit the existing `session_started|ended|failed` events with a
   `port_id` field instead of `edition_id`.
5. API: `port_sessions { port_id? }` returning totals and the last 50 rows.
   `list_ports` gains `playtime_seconds`, `play_count`, `last_played_at`.
6. UI: `unified.ts` fills `playtimeMin`/`lastPlayed` for ports; Game page shows
   playtime instead of "Not tracked".

**Tests.** Fake port binary (a shell script fixture used by existing core
tests) that sleeps then exits: session completes and credits totals; exit
under 10 s fails; a second launch while running is refused; recovery after a
simulated restart closes the session as interrupted. Desktop: `unified.ts`
maps the new fields.

**Acceptance.** Owner launches Shipwright, plays past 10 s, quits; playtime
appears and survives a restart. Record if not performed.

**Out of scope.** Streamed port play; save handling.

## NP2 — Port data readiness

**Why.** A port that is installed but has no ROM shows Play and fails inside
the engine.

**Current state.** `port_engine::info` (`:109-120`) reports `ready` when the
engine is installed. `presentation.ts` already renders `needs-rom` and
`needs-data` but nothing sends them.

**Change.**
1. Add an optional `data` block to `PortDef` and each `catalog/ports/*.json`:
   `{ "kind": "rom" | "data", "files": [{ "path": "...", "sha256": [..] }],
   "any_of": [...] }`. Paths are relative to the port directory. Hashes are
   the known-good dumps listed by each port's upstream docs; record the source
   URL in a `source` field. Where upstream accepts several dumps, list them all.
2. `info` checks presence first, then hashes (stream, bounded, cached by
   size+mtime in the existing settings key) and reports `needs-rom` or
   `needs-data` when nothing matches. Keep the existing `review` precedence
   (an installed engine needing data and review shows data first).
3. `state.svelte.ts` `applyPorts` passes the new statuses through instead of
   mapping to `installed`. `canPlay` stays limited to `ready`.

**Tests.** Per-port fixture: missing file → `needs-rom`; wrong hash →
`needs-rom`; matching synthetic hash (inject the expected hash in a test def)
→ `ready`. Every shipped recipe has a `data` block with a source URL.

**Acceptance.** UI shows "Needs ROM" for an installed port without data.

**Out of scope.** Copying or generating data (NP3/NP4).

## NP3 — Shipwright game-data setup job

**Owner guide:** [Quiver Q2c](QUIVER_INTEGRATION.md), [emulators: Ocarina pilot](EMULATORS.md).

**Why.** Shipwright needs an O2R asset archive generated from a supported OoT
ROM. Today the owner does this manually inside the engine.

**Current state.** Manual drop into `SHIP_HOME = <data>/ports/shipwright`. The
PAL 1.1 ROM used in the pilot matches Shipwright 9.3.1's supported list.

**Change.**
1. Manager job kind `port_setup` in `native_ports.rs` (mirror `Request::Start`
   / `Cancel`, `jobs.db` persistence, checkpoints).
2. Input: an owner-picked ROM path. Verify against NP2's hash list; refuse
   unknown dumps with the expected list.
3. Copy the ROM into the port directory (never move the original). Run the
   pinned engine's own extraction if it has a headless mode; if it does not,
   stop at "ROM placed, first launch will generate assets" and record that.
   Check the engine's CLI before writing code; do not reimplement extraction.
4. Write a setup receipt (`recipe`, ROM sha256, outputs with sha256) beside the
   install receipt.
5. UI: `PortDialog.svelte` shows "Choose ROM" when status is `needs-rom`.

**Tests.** Job with a synthetic ROM whose hash is injected: copied, receipt
written, original untouched; cancel mid-copy leaves no partial file; restart
recovers an interrupted job.

**Acceptance.** Owner selects the real ROM, sees `ready`, plays to file
select, saves, relaunches and loads the save. Record if not performed.

## NP4 — Data setup for the other ports

One change set per port, same job and receipt as NP3, with each port's own
rule from its `requirement` text and upstream docs: 2Ship2Harkinian (MM ROM
beside binary), Zelda64Recomp (MM ROM through its setup), sm64coopdx
(`baserom.us.z64`), Fallout 2 CE (copy `master.dat`, `critter.dat`, `data/`
from an owned install; no hashes for mods), UZDoom (owner WAD, or Freedoom
from a pinned release). Acceptance per port as in NP3.

## NP5 — Port release review screen and re-pin script

**Why.** `review` now blocks unreviewed upstream releases but gives no path
forward.

**Current state.** `check_port_updates` stores `port.<id>.latest`
(`lib.rs:~541-558`). Pins are compiled in (`port_engine.rs:19-26`).

**Change.**
1. `scripts/repin-port.py <id> <tag>`: reads `catalog/ports/<id>.json`,
   fetches the release through the GitHub API, selects the asset by the
   recipe's `asset_suffix`/`asset_excludes`, downloads it, verifies the GitHub
   digest when present, computes `sha256`, extracts and computes
   `binary_sha256` (ELF x86-64 check), writes the JSON in place and prints the
   diff. No commit.
2. Port dialog for `review`: installed tag, upstream tag, link to the
   upstream release notes, and the text "A Marquee update must review this
   release before it can be installed."
3. Document the maintainer flow in `docs/EMULATORS.md` beside the port table.

**Tests.** `scripts/tests/test_repin_port.py` against a local HTTP fixture
(asset, nested tar/zip, digest present and absent, wrong digest refused).
Desktop: presentation test for the review dialog text.

**Out of scope.** Runtime pin overrides (FD3).

## PS4-1 — shadPS4 launch contract and runner

**Owner guide:** [console preparation](CONSOLE_PREPARATION.md#isolated-ps4-preparation-spike-2026-10-25).

**Change.** Verify the pinned shadPS4 v.0.20.0 CLI for a prepared game path
(the eboot or game folder), fullscreen and config/user directory flags. Add
`kind: "emulator"` and a `launch` template to `catalog/emulators/shadps4.json`
only once the contract is confirmed; keep `-b` for Tools/open. Isolate config,
saves and shader cache under `<data>/shadps4/` like `launch_command` does for
others. Record the contract in the guide.

**Tests.** Catalogue test: shadPS4 becomes a runner; launch argv uses the
prepared path and isolated directories; the existing runner test
(`emulators.rs:~1387`) updated.

## PS4-2 — PS4 base-game preparation recipe

**Change.** Manager recipe `ps4_pkg_base` reproducing the operator spike as a
durable job: pinned ShadPKG v2.1.0 (`8fb98f60…cd8ee`) via `download_verified`;
run in bubblewrap with no network, read-only source/tool, single writable
output; inputs limited to base packages identified by `console::inspect_pkg`
(category `gd`). Output receipt lists files and the eboot SELF candidate.
Updates/DLC are recorded as separate inputs and not overlaid in this item.

**Tests.** Recipe refuses non-base packages and packages whose title IDs
disagree; synthetic package fixture for inspection; job cancel and restart.

**Acceptance.** CUSA08809 base prepares into a managed installation.
Shadow of the Colossus is recorded as unplayable in shadPS4, so playability
needs a different owner title.

## PS4-3 — PS4 system descriptor and Library Play

**Change.** `catalog/systems/ps4.json` (mirror `ps3.json`), scan kind and
`library.rs` registration from the PS4-2 receipt (mirror `xbox360_iso`),
title ID from SFO, Play through the PS4-1 runner. Update the headless rollout
table only to note "local first".

**Acceptance.** Owner title boots, accepts input, saves, exits, relaunches.

## PS5-1 — KytyPS5 pin and launch contract

**Change.** Pin `KytyPS5-2026-10-25-83f8bfd` (or newer, re-verified) with
`download`/sha256 in `catalog/emulators/kytyps5.json`. Confirm
`--game <directory|elf|zar>`, profile and save locations. Record the contract.
No runner yet.

## PS5-2 — PS5 preparation and Library Play

**Change.** Parse `sce_sys/param.json` inside the inventory to confirm the
PPSA identity; a manager recipe extracts a prepared folder from the owner's
archive (bounded, traversal refused, 87 GB declared output checked against
free space first); then system descriptor, registration and runner as in
PS4-3. Never pass the RAR to Kyty.

**Acceptance.** As PS4-3, with Demon's Souls or another owner title.

## X360-1 — Xbox 360 save/relaunch acceptance support

**Change.** Add a Tools/Game action that lists Xenia content folders for the
edition's title ID under `<data>/xenia/xdg-data/content` with modified times,
so the owner can confirm a save was written. Align the Xbox 360 row in
`docs/EMULATORS.md` with the integrated runner (the guide still says it does
not register a runner).

**Acceptance.** Owner saves in Vanquish, quits, relaunches, loads; result
recorded in the console guide.

## ST3a — Wolf runtime controls in the app

**Owner guide:** [streaming ST3](STREAMING.md), [viewer](STREAMING_VIEWER.md#streaming-screen).

**Change.** API commands `streaming_runtime_status`, `streaming_runtime_build`
(background, cancellable via the existing `AtomicBool`), `streaming_runtime_probe`
and `streaming_runtime_rollback` (confirmed), wrapping `runtime.rs`. Streaming
screen "Host runtime" section: receipt, build with progress, probe result,
rollback. Keep the no-Start-button rule until a host start operation exists;
library headless already owns its own host.

**Tests.** API tests with a fake `podman` on `PATH` (pattern used by streaming
integration tests in `crates/core/tests/streaming.rs`); desktop state test for
the section.

## ST3b — Base image independent of the Xenia pilot

**Change.** Genesis can reuse the Sonic/BlastEm pilot or compose the installed
Wolf and managed BlastEm images. Image selection checks local availability.
Each library adapter caches its result against the immutable parent and recipe.
Existing Xenia pilot installs and legacy library base receipts are supported.

**Tests.** Cover missing images, managed composition and adapter-specific selection.

## ST3c — Streamed playtime

**Change.** Schema v7 records streamed games with `mode = 'stream'`. The
emulator process lock in the configured streaming folder confirms startup and
continued play. Reconnect reuses the running row. Recovery credits the last
saved duration when the process has ended.

**Tests.** Cover migration, refusal during local play, reconnect, recovery in a
custom streaming folder, exit and checkpoint credit.

## HL1 — Mesen headless adapter

**Change.** Allowlist arm for Mesen systems in `headless_plan`; `mesen`
branch in `library-game.py`; package the managed Mesen build in the library
image; per-edition streaming profile like BlastEm. Update the rollout row in
[headless streaming](HEADLESS_STREAMING.md#rollout).

**Acceptance.** Owner streams an NES/SNES title: video, audio, keyboard,
save/state restore, reconnect, exit.

## HL2 — DuckStation and PCSX2 headless adapters

One change set each, after HL1 settles the Qt display route. BIOS and memory
cards come from the existing managed locations, mounted read-only (BIOS) and
read-write (cards, streaming profile copy).

## ST4 — Embedded receiver research spike

**Change.** Decision record only, in `docs/STREAMING.md` under ST4: options
(a native receiver on moonlight-common-c with a Rust video pipeline, a Tauri
window hosting a native surface, a WebRTC bridge), Wayland constraints,
licensing (GPL-3.0 Moonlight core), effort and a recommendation. No code.

## ST5 — Configurable stream target and opt-in LAN publish

**Change.** `Connection` gains `target_host` (default `127.0.0.1`), passed to
`viewer::view`/`pair` (`viewer.rs:70-72`, `:120-123`). A setting
`streaming.lan_address` (default unset) makes the headless container publish
on that one address and sets `WOLF_INTERNAL_IP`. Settings shows the exposed
ports and a firewall note.

**Tests.** Default plan publishes only on loopback; LAN plan publishes only on
the chosen address; `0.0.0.0` refused.

**Acceptance.** ST5 gate in the streaming guide, from a second device.

## S1A — Installed Steam library

**Owner guide:** [S1A task list](STORE_INTEGRATION.md#s1a-installed-steam-games). Follow its eight tasks; this section adds anchors and slicing.

1. Persist: additive `steam_installations` table keyed by the stable
   `Installation.id` from `steam/mod.rs:145`; transactional rescan that keeps
   IDs and user edits and marks missing rows instead of deleting.
2. Project into `LibrarySnapshot` as `source: 'steam'` with install state;
   reuse `catalog_links` for artwork when present.
3. Launch: `open_steam_game(installation_id)` hands off to the owning client
   (`steam -applaunch <appid>` for native, the Flatpak run form for Flatpak),
   resolved from the inventory's `Client`. No guessed `.exe`.
4. Rescan on focus and after handoff, bounded.
5. Desktop: Library filter and Game page actions; remove the cosmetic
   `steam://rungameid` label in `GamePage.svelte:~49` or back it with the real
   handoff.

**Acceptance.** The S1A increment gate in the guide.

## S1B — Owned Steam library sync

Follow [S1B](STORE_INTEGRATION.md#s1b-owned-steam-library-and-installation-handoff).
Verify the Web API route first and write the finding into the guide before
code. Consume the existing `steam_web_api`/`steam_id` keys server-side only.

## C1 — Shared managed Windows runtime

**Change.** Extract from `purple.rs` a `windows_runtime` module: pinned umu
(`UMU_URL`/`UMU_SHA`), pinned Proton build, receipts, managed prefixes under
`<data>/windows/prefixes/<id>/`, in-use lock, rollback. Purple keeps its
existing paths and receipt `RECIPE` string by delegating to the new module.
Then [C1 tasks 5–6](STORE_INTEGRATION.md#c1--e1-compatibility-and-epic--second-store-milestone).

**Tests.** Existing Purple tests unchanged; new tests for prefix creation and
in-use refusal.

## E1 — Epic through Legendary

Follow [C1 + E1](STORE_INTEGRATION.md#c1--e1-compatibility-and-epic--second-store-milestone)
tasks 1–4 and 7–9. Slice: (1) pin and status, (2) sign-in/out with tokens in
backend storage, (3) owned sync into the S1A-style store table, (4) install
job in the manager, (5) launch through C1 with lifecycle evidence.

## G1 — GOG through gogdl

Follow [G1](STORE_INTEGRATION.md#g1-gog--third-store-milestone) with the same
slicing as E1. Native Linux builds launch directly.

## MP0 — Portable package manifest

**Owner guide:** [portable packages](PORTABLE_PACKAGES.md).

**Change.** `crates/manager/src/portable.rs` with a versioned `Manifest`
holding the fields the guide lists, embedding the existing manager `Receipt`
as `output_receipt`. Synthetic fixture `crates/manager/tests/fixtures/portable-v1.json`.
No local instance IDs in portable identity.

**Tests.** Round-trip, unknown major version refused, absolute paths and `..`
refused.

## MP1–MP3

Follow MP1–MP3 in the guide. MP1 export/import are manager jobs that verify
every file hash and register through the existing receipt publication.
Saves stay opt-in and separate.

## SL1 — Reviewed source cleanup candidates

**Owner guide:** [collection storage](STORAGE_LAYOUT.md), [storage manager](STORAGE_MANAGER.md).

**Change.** API `storage_cleanup_review` returning originals whose receipts
show a verified managed installation, with sizes and receipt IDs. Deletion is
a separate confirmed command that re-verifies the installation hashes first
and moves the original to the desktop trash rather than unlinking.

**Tests.** Candidates exclude sources without a verified receipt; delete
refuses when a hash no longer matches.

## SL2 — Cross-filesystem installation moves

Extend `relocation.rs` with copy, verify all hashes, switch and journal, then
remove the source only after the switch commits. Rollback before commit
deletes the copy. Same review/apply/rollback API.

## UL1 — Normalized identity model

Follow the [unified library](UNIFIED_LIBRARY.md) task list. First change set:
schema v8 tables plus a backfill where each existing edition becomes one game,
one edition and one installation, with a migration fixture from a v7 database
and no UI change.

## F1 — Playtime history and recently played

**Change.** API `game_sessions { edition_id }` (and the NP1 equivalent for
ports) returning recent sessions and per-week totals. Game page "History"
section: last sessions with date, duration, interrupted/failed state. Home
"Continue playing" already sorts by `lastPlayed` (`Home.svelte:20`); add a
"This week" stat from the sessions list.

**Tests.** Aggregation across week boundaries in local time; failed sessions
excluded from totals.

## F2 — Save snapshots and restore

**Change.**
1. `crates/core/src/snapshots.rs`: snapshot a scope (`emulator:<id>` →
   `<data>/<id>/xdg-data` and `xdg-config`; `port:<id>` → the port directory
   minus files listed in its install receipt) into
   `<data>/backups/<scope>/<utc>.tar.gz` with a JSON manifest of paths, sizes
   and sha256 (`tar` and `flate2` are already workspace dependencies).
2. Automatic snapshot before `emulators::install` replaces `current/` and
   before port engine promotion. Keep the newest five per scope.
3. Restore: refuse while that emulator or port is running; snapshot current
   state first; extract to a staging directory; verify the manifest; swap.
4. Settings → Storage lists snapshots per scope with restore and delete.

**Tests.** Round-trip preserves bytes and modes; restore refused while
running; restore creates a pre-restore snapshot; retention keeps five;
symlinks inside a scope are stored as links and never followed on restore.

## F3 — Soundtrack downloads (MU4)

**Owner guide:** [soundtracks MU4](SOUNDTRACKS.md), [YouTube OST](YOUTUBE_OST.md).

**Change.**
1. Share the yt-dlp pin and safe flags between core and manager (move
   `VERSION`, `SHA256`, helper install and base args into `marquee_media`).
2. Manager job `music_acquire`: input is an already linked album source;
   per-track rows with state, bytes, cancel, retry; yt-dlp with the safe
   flags plus `-f bestaudio --no-playlist` per track into a staging folder.
3. On completion, hand files to the existing `music_import` path so storage
   stays content-addressed, and attach tracks to the album.
4. UI: "Download tracks" in the soundtrack panel, per-track progress.

**Tests.** Fake yt-dlp on `PATH` producing files: job completes, imports,
cancel and retry per track, restart recovery.

## F4 — Controller inventory and native navigation

**Owner guide:** [controllers](CONTROLLERS.md#marquee-follow-up-input-profiles).

**Change.** Add `gilrs` to the Tauri crate; emit connect/disconnect and
button events to the UI; `focus.ts` consumes them alongside the browser
Gamepad API (its comment already reserves this). Settings lists connected
controllers. No emulator binding writes in this item.

**Acceptance.** Deferred until the owner has a controller; record as
unperformed.

---

## Implementation log

### 2026-11-25

Defaults FD1–FD9 accepted. Implemented NP2, NP5, X360-1, MP0, F1 and ST4.
Live Xbox 360 save/relaunch was not played. No commit was made in this pass.

- **NP2.** Each port recipe declares `data`. An installed engine reports
  `needs-rom` or `needs-data` until a file in its folder matches a published
  hash, or, for Fallout 2, until `master.dat`, `critter.dat` and `data/` exist.
  Freedoom 0.13.0 WAD hashes were checked against that GitHub release zip.
  Retail Doom IWADs are accepted by filename. A symlink that leaves the port
  folder does not count. Missing data outranks `review`. The UI no longer
  renames backend `ready` to `installed`.
- **NP5.** `scripts/repin-port.py` rewrites one pin and prints the diff.
  The port dialog tells the owner that a newer tag waits for a Marquee update.
  Documented in [emulators](EMULATORS.md).
- **X360-1.** `xenia_saves` lists real directories under
  `xenia/xdg-data/content` for the edition's title id. The game page shows them.
  The emulator guide no longer says Xenia has no library runner.
- **MP0.** `marquee_manager::portable::parse` accepts schema 1, embeds a
  preparation receipt, and rejects absolute paths, `..` and schema 2.
  Fixture: `crates/manager/tests/fixtures/portable-v1.json`. Identity ignores
  the receipt destination.
- **F1.** `game_sessions` and `play_summary` credit ended sessions in the local
  week, starting Monday. Failed and running sessions are excluded. The game
  page lists recent sessions. Home shows this week's time once it reaches a
  minute.
- **ST4.** Decision recorded in [streaming](STREAMING.md#st4--embedded-receiver-decision-2026-11-25).
  No receiver code.

### 2026-11-26

- **NP1.** Native ports record `port_sessions` instead of borrowing `sessions`.
  A port and a library game cannot run together, and a streamed game cannot
  start while either is running. Exits under 10 seconds are failed starts.
  A port left running when Marquee exits becomes `interrupted` and is not
  credited. Ended time appears on the port's library card and history.
  Live Shipwright play was not performed.

- **F2.** Emulator profile folders and port saves (not the port engine) are
  copied to `<data>/backups` before an emulator install replaces `current/`
  and before a port engine install starts. Storage lists the newest five per
  scope. Restore refuses while that game is running, copies the current files
  first, and does not follow symlinks.

- **ST3a.** The Streaming screen can build, cancel, probe and roll back the
  managed Wolf image. It still has no button that starts a host. A non-rootless
  Podman answer stops the build. Probe and NVIDIA probe run in the background
  and cannot overlap a build. A failed rebuild still shows the last good receipt.

- **ST3b.** Genesis reuses the Sonic/BlastEm image or builds it from managed
  Wolf and BlastEm. Existing Xenia pilot installations remain supported.
  Each adapter caches the derived library image with its own receipt.

- **F1 follow-up.** Home “this week” includes ended port sessions. A port
  engine install refuses while that port is running.

- **PS4-1.** shadPS4 registers `runner.shadps4.marquee` with `-g` on the
  prepared path, `-f true|false`, isolated XDG folders, and Tools still using
  `-b`. No `ps4` system descriptor yet. Live boot unperformed.

- **PS5-1.** KytyPS5 installs from the pinned
  `KytyPS5-2026-10-25-83f8bfd` Linux tarball (GitHub digest
  `737ea99e…b6d7e9`). `--game` is recorded; save/profile paths and a runner
  were not added. Live `--help` unperformed.

- **ST3c.** Schema v7 adds `sessions.mode` (`local` default). A headless
  launch records `mode='stream'` after observing the game process lock in the configured streaming folder.
  Recovery credits the last confirmed checkpoint if the process has ended. Viewer close does not end the row. Ended stream time is credited and
  shown as `x h (y h streamed)`. Live stream playtime was not performed.

- **NP3.** Shipwright `needs-rom` offers Choose ROM. The manager copies a
  matching dump into the port folder (original untouched), writes
  `.marquee-setup.json`, and records that first launch generates assets.
  Unknown dumps are refused. Live ROM setup was not performed.

- **NP4 (2ship2harkinian).** Majora’s Mask uses the same ROM copy job as
  Shipwright against its published SHA-1 list. Live ROM setup unperformed.

- **Shipwright live acceptance (NP1, NP3, F2).** A fresh library behind
  headless gamescope, driven by injected keyboard and pointer input. The manager
  installed pinned 9.3.1, the setup job placed the PAL 1.1 ROM, and on first
  launch Shipwright's own dialogs generated `oot.o2r` (three clicks; the setup
  job cannot do this). File 1 was created, played and saved in-game, then the game
  was quit, relaunched and the file loaded. A snapshot was taken, File 1 was erased
  in-game, and the restore brought it back byte-for-byte. Restore while running was
  refused. Four launches gave four `ended` rows with exit 0, and the week total
  matched. The first row includes the minutes spent in the asset dialogs.
  Fixes from the run:
  - port snapshots leave out the placed ROM (receipt outputs) and generated
    `.o2r`/`.otr`, so they drop from 53 MB to about 4 KB, and an old snapshot cannot put
    a previous engine's assets back;
  - restore keeps `.marquee-setup.json` and other `.marquee-*` files;
  - restored files keep their original mtime instead of 1970;
  - port session history returns `ended_at` and `exit_code`.
  A physical controller and listening to the audio were not checked; the
  `soh.elf` stream was running in PipeWire.
