# PlayStation (PS1 slice)

Current correction notes: [PS1/PS2 correction pass](CORRECTIONS.md). Platform mapping, CUE handling and launch access have changed since the historical checks below. Live gameplay acceptance remains separate.

Last updated: 2026-10-21.

This document covers the **PS1 vertical slice**: a second system beside PS2, using the same library, session supervisor, and CLI. PS2 remains the reference slice and the current release gate — see [PS2.md](PS2.md). Long-term phases: [ROADMAP.md](ROADMAP.md). Other systems: [EXPANSION_CATALOGUE.md](EXPANSION_CATALOGUE.md).

Release boundary:

> A persistent PS1 library that imports `.iso` and `.chd`, matches serials for art, and launches through an installed DuckStation, recording real session duration.

That gate is **not closed**. Everything below is verified on this host except the live DuckStation Play → quit → duration, because DuckStation is not installed here.

Marquee does not download BIOS files or commercial games.

---

## What changed from the PS2-only build

| Concern | Before | Now |
| --- | --- | --- |
| System descriptors | one hardcoded `ps2.json` | `catalog/systems/*.json` behind `catalog::by_id` / `catalog::all` |
| Library roots | a root had no system | `library_roots.system` (schema **v4**), set at `add-root` |
| Disc formats | `.iso` only, by extension | per-system supported/skipped sets; PS1 also imports `.chd` and `.cue` |
| Disc layout | 2048-byte sectors assumed | the ISO9660 reader probes 2048 / 2352 (Mode 1) / 2352 (Mode 2) / 2336 strides |
| Runner | PCSX2 only | `RunnerKind::Duckstation` beside `RunnerKind::Pcsx2` |
| Runner choice | one global pin | scoped to the title's emulator family |
| Covers | xlenore/ps2-covers | per-system set (`psx-covers` for PS1) |

### Disc layout (why raw images now work)

Classic PS1 discs are usually ripped **raw at 2352 bytes per sector**, not the 2048-byte user-data layout of a `.iso`. Marquee probes each stride and reads `SYSTEM.CNF` from the first layout whose ISO9660 Primary Volume Descriptor validates:

| Stride | User data at | Used by |
| --- | --- | --- |
| 2048 | sector + 0 | `.iso` rips (both PS2 and PS1) |
| 2352 | sector + 16 | Mode 1 raw dumps |
| 2352 | sector + 24 | Mode 2 Form 1 raw dumps (common for PS1) |
| 2336 | sector + 12 | Mode 2 without sync |

Both `BOOT` (PS1) and `BOOT2` (PS2) lines are accepted, so one reader serves both.

`.chd` is compressed and is not ISO9660 on disk, so it imports with no serial (DuckStation reads it directly).

### CUE/BIN pairs

A `.cue` sheet plus a `.bin` track is the most common PS1 rip. The **`.cue` is what
imports and launches**; the `.bin` is counted as skipped, because importing the track
separately would list one disc twice. Marquee parses just enough of the sheet
(`FILE` / `TRACK` / `INDEX 01`) to find the **data track** — skipping a leading `AUDIO`
track if present — and reads the serial from that track's own ISO9660 filesystem.

The `FILE` name is reduced to its final path component and resolved beside the sheet, so a
crafted sheet cannot point the reader outside its own folder. Tests cover that, plus
audio-before-data ordering and `INDEX 00` not counting as a track start.

A `.bin` with no matching `.cue` is skipped, not guessed.

Both discs of a multi-disc game import as separate editions, keyed off their own serials.

### Serial matching

`BOOT = cdrom:\SLUS_005.94;1` normalises to `SLUS_005.94` and the cover filename `SLUS-00594.jpg`.

Not every PS1 disc carries a serial. Homebrew and Net Yaroze discs often boot an `.EXE` instead:

```text
BOOT=cdrom:\doukutsu.exe;1
BOOT=cdrom:\START.EXE;1
```

Those import with `serial: null` and `region: null`, keep procedural art, and are counted under `unresolved_serial` in the scan report. That is the correct outcome, not a parse failure.

---

## How to run

```bash
cd apps/desktop
pnpm install
pnpm tauri dev        # or ./scripts/run.sh desktop on NVIDIA Wayland

# CLI, isolated data dir
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev library add-root --system ps1 ~/ROMs/ps1
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev scan
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev list --system ps1
cargo run -p marquee-cli -- --data-dir /tmp/marquee-dev launch <edition-id>
```

Unknown systems are rejected at `add-root` with `ROOT_OFFLINE` (`known: ps2, ps1`).

### Launch argv

```text
duckstation-qt -batch -fullscreen -nogui /path/to/game.iso
flatpak run org.duckstation.DuckStation -batch -fullscreen -nogui /path/to/game.iso
```

DuckStation does **not** take PCSX2's `--` separator; the image is a positional argument.

### Emulator choice

The runner is chosen from the **edition's system**:

- PS1 → `Duckstation`, else the fake test runner
- PS2 → `Pcsx2`, else the fake test runner

A stored `selected_runner_id` is honoured only when it belongs to that family (or is the system-agnostic fake runner). Without this, a PCSX2 pin left by a PS2 session would be used to boot a PS1 disc.

### BIOS / setup status

DuckStation needs a PS1 BIOS, and the same three states apply (`missing` blocks Play; `unverified` does not). The Flatpak path probed is `~/.var/app/org.duckstation.DuckStation/config/duckstation`.

Firmware acquisition jobs are planned ([media acquisition](MEDIA_ACQUISITION.md)). Today: `marquee bios import <folder>` or Tools import for managed DuckStation (`~/.local/share/marquee/duckstation/xdg-config/duckstation/bios`). External Flatpak DuckStation still uses its own BIOS UI. See [Managed emulators and BIOS](EMULATORS.md).

### Errors

| Code | What to do |
| --- | --- |
| `RUNNER_MISSING` | Install DuckStation (see below). |
| `SETUP_MISSING` | Queue BIOS setup (when available) or import a folder in Tools. |
| `MEDIA_MISSING` | Disc moved, or the format is not launched (`.cue`/`.bin`). |
| `ROOT_OFFLINE` | Folder missing/unmounted, or an unknown `--system`. |

---

## Verified on this host (2026-10-21)

Real library (`~/.local/share/marquee/library.db`), PS1 root `./roms/ps1`:

| Check | Result |
| --- | --- |
| `library add-root --system ps1` | **Pass** — root stored with `system: ps1` |
| Scan of a mixed folder | **Pass** — a `.cue`/`.bin` pair imports once (via the `.cue`), the track is skipped |
| **Metal Gear Solid Disc 1** (real disc, `.cue`+`.bin`, Mode2/2352) | **Pass** — serial **`SLUS_005.94`**, region NTSC-U |
| **Metal Gear Solid Disc 2** (real disc, `.cue`+`.bin`, Mode2/2352) | **Pass** — serial **`SLUS_007.76`**, region NTSC-U, separate edition |
| Raw 2352 reading | **Pass** — Mode 2 Form 1 stride (`2352*16+24`) validates on both real MGS discs, whose `.cue` declares `MODE2/2352` |
| PS1 launch with a PCSX2 pin present | **Pass** — reports `RUNNER_MISSING: no emulator is installed for ps1`, not PCSX2 |
| PS2 launch with the same pin | **Pass** — unchanged, still selects PCSX2 |
| `scripts/check.sh` Rust half | **Pass** — fmt, clippy `-D warnings`, **79 core tests** |
| `pnpm check` / `pnpm test` / `pnpm build` | **Pass** — 0 type errors, 31 UI tests, build |

### Not verified

| Check | State |
| --- | --- |
| DuckStation discovery (unpacked / Flatpak) | **Not installed on this host** — code path only |
| Live PS1 Play → quit → duration | **Pending** |
| DuckStation BIOS probe | Code path only |
| PS2 serial covers / PS2 Play gate | Still pending, unchanged — see [PS2.md](PS2.md) |

The homebrew titles (Doukutsu, Net Yaroze ×3) import with `unresolved_serial` — expected, and
the reason no covers were fetched for them.

### Caveat: the PS1 cover set is incomplete

`xlenore/psx-covers` holds ~9,100 covers under `covers/default/`, but it is **not one per
serial**, and a missing serial returns a 14-byte text placeholder rather than a 404. Marquee's
JPEG magic-byte check already rejects that placeholder, so a miss degrades to procedural art
instead of a broken image — but it counts as `skipped`, not `failed`.

Both Metal Gear Solid discs resolve, and they serve the **same image** (identical MD5,
`7410c8a2…`, 500×500). Sampled neighbouring serials (`SLUS-00595`, `SCUS-94163`, `SLUS-00001`,
`SLPS-00001`) return **distinct** images, so this is specific to this pair rather than a
uniform placeholder. Treat a PS1 cover as "a" cover, not necessarily the exact disc art.

---

## Code map

| Claim | Where |
| --- | --- |
| System descriptors | `catalog/systems/ps1.json`, `catalog/systems/ps2.json`, `crates/core/src/catalog.rs` |
| Schema v4 (`library_roots.system`) | `crates/core/src/db.rs` (`SCHEMA_VERSION`, `column_exists`) |
| Media kinds + runner kinds | `crates/core/src/model.rs` |
| Sector-stride ISO9660 reader | `crates/core/src/iso.rs` |
| CUE sheet parsing + track resolution | `crates/core/src/cue.rs` |
| System-aware scan/ingest | `crates/core/src/library.rs` |
| DuckStation discovery, argv, BIOS | `crates/core/src/runners.rs` |
| Family-scoped runner selection | `crates/core/src/runners.rs` (`selection_for`), `crates/core/src/sessions.rs` |
| Per-system covers | `crates/core/src/artwork.rs` |
| Command layer | `crates/core/src/api.rs` (`add_library_root` takes `system`) |
| CLI | `crates/cli/src/main.rs` |
| UI folder buttons | `apps/desktop/src/lib/state.svelte.ts` (`addSystemFolder`), `screens/Library.svelte` |
| Tests | `crates/core/src/tests.rs`, module tests in `iso`, `artwork`, `catalog` |

---

## Next

1. Install DuckStation (Flatpak `org.duckstation.DuckStation`, or an unpacked build) and close the live Play gate.
2. Add a PS1 BIOS in DuckStation; confirm `missing` → `unverified`/`launch-tested`.
3. Confirm covers for a disc that *does* carry a serial.
4. Still PS2-only for now: `.cue`/`.bin` pairing, multi-disc, `.pbp`.
