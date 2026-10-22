# Correction history

Condensed record of the 2026-10-20 correction pass and the 2026-10-21 PS1/PS2 audit and correction. Detailed audit findings were removed once fixed; see git history for the originals. This file does not reorder the [roadmap](ROADMAP.md) or close any live gate.

## 2026-10-20 — runtime/session correction pass

| Issue | Correction |
| --- | --- |
| ZIP/TAR extraction followed directory symlinks | Reject symlink traversal; reject release/user symlink conflicts before promotion |
| Restart lost a live emulator session and allowed a second launch | Persist PID + boot/start identity; checkpoint monotonic duration; recover and monitor matching processes; hold the instance lease while monitored |
| Failed update partially replaced the executable | Stage, validate and rename with backup/rollback; saves preserved |
| Nested TAR update kept the old binary | Unpack only into the fresh release directory before promotion |
| Global runner choice ignored after first launch | Stop creating implicit per-edition profiles; v3 migration clears legacy auto-created profiles (titles, favourites, sessions, playtime kept) |
| Other windows/tabs missed library mutations | Emit `LibraryChanged` after root/title/favourite/runner changes |
| Truncated ISO aborted a whole scan | Per-file import warnings; fatal DB errors still abort |

Also: explicit instance-flock release, atomic session finalize + playtime credit, time kept when a game exits unsuccessfully, native window hidden (not closed) during a tracked game.

Schema v3 adds `sessions.process_identity`. Known limits: recovery is Linux-specific (`/proc` + boot ID); a dead process's exit status/exact time is unavailable, so the last checkpoint is credited and the session reported interrupted; a small spawn-before-identity-write window remains; staged install rollback is not atomic across renames and SQLite on power loss (update-journal recovery is follow-up); native ports do not yet get session tracking.

## 2026-10-21 — PS1/PS2 audit and correction

The audit used the real collection read-only: **11 disc entries (9 PS1, 2 PS2)**, 15 recognised files → 11 entries + 4 skipped BIN companions, 7 unresolved identities, IDs stable across restarts. Retail serials and region matched for Vice City, both MGS discs and Silent Hill.

### Findings fixed

| Finding | Correction |
| --- | --- |
| F1 — every PS1 disc displayed/filtered as PS2; PCSX2 offered for missing DuckStation | System and runner carried through mapping; readiness, setup actions and defaults resolved per system (`selected_runner.ps1` / `.ps2`); Settings split by system |
| F2 — CUE with missing track files still looked available | Manifest validates every referenced track (incl. audio); entry stays with a named diagnostic, Play blocked, restoring the file restores it |
| F3 — replacing a BIN kept the cached identity | CUE identity re-read from bounded metadata each scan; changed identity detaches stale artwork |
| F4 — CUE reader ignored data-track `INDEX 01` offset | Offsets honoured; multi-sector ISO9660 reads skip raw headers; relative subfolders resolved, absolute/parent/escaping symlink paths rejected |
| F5 — no native/AppImage DuckStation discovery | PATH commands and executable AppImages in Applications, `.local/bin`, Downloads discovered alongside Flatpak |
| F6 — failed launches counted as plays; exit 0 marked BIOS `launch_tested` | Known-failed exits keep their record but add no play count/playtime/last-played; exit 0 no longer upgrades BIOS readiness. Existing totals were not rewritten |
| F7 — re-adding a root under another system silently kept the old one | Returns `ROOT_SYSTEM_CONFLICT`; no implicit reclassification |
| PCSX2 Flatpak could not read the Vice City ISO | Flatpak launches get a temporary read-only grant for the ISO/CHD or CUE folder; saved overrides unchanged |

### Still open

- **Live gates not closed.** PS2: Vice City with correct runner + BIOS + folder access → boot → input → exit → duration persisted once → restart → relaunch. PS1: Silent Hill or MGS Disc 1, same sequence. Helengine is only a smoke test.
- **Machine state at audit time:** no DuckStation installed; PCSX2 Flatpak 2.8.2 had an empty BIOS folder. Re-check before assuming readiness.
- Multi-disc identity (M3U / disc swap), explicit CHD↔CUE alternative-installation linking, and CHD serial extraction are unimplemented.
- BIOS probing gives hints only; it does not parse the configured BIOS selection or surface the sandbox failure.
- Full game-boot detection and crash-aware gameplay accounting are future work.
- Controller/renderer surfaces and real gameplay were not tested; unit tests passing is not release acceptance.
- Historical statistics from earlier failed sessions need an explicit reconciliation policy, not a silent reset.

## 2026-10-21 — hands-off emulator setup

PCSX2's first launch opened its setup wizard and exited under `-batch -nogui` (`SPAWN_FAILED`, exit 1). Marquee now writes the minimal valid `PCSX2.ini` itself, seeds DuckStation's `SetupWizardIncomplete`, imports BIOS files/zips from `bios-inbox`, and ignores host (Flatpak, system, loose AppImage) emulators. After this, a live PCSX2 launch of Vice City (USA) ended `exit 0` after 254 s with the session stored. The earlier "PCSX2 Flatpak could not read the ISO" and DuckStation AppImage-discovery findings (F5) are superseded: those code paths were removed. Note: building with a plain `cargo build` here may write to a temporary `CARGO_TARGET_DIR`; use `CARGO_TARGET_DIR=$PWD/target` to refresh `target/release/marquee`.

## 2026-10-22 — RAR intake validation

RAR inspection now uses the packed UnRAR ABI: the high-level wrapper discarded
redirection metadata, and its underlying Rust structs used padded layouts that
misread later native fields. RAR4 Unix symlinks and RAR5 redirections (symlinks,
hardlinks, junctions, file copies) are rejected before staging. UnRAR runs in
test/decompression mode with a bounded write callback, never filesystem
extraction mode. Marquee creates regular files and rejects CRC/size errors.
Synthetic stored RAR4/RAR5 fixtures exercise successful extraction, links and
traversal; the callback has an oversized-output regression test. The frontend
acceptance test and import labels now agree with RAR support.

## 2026-10-22 — provider CUE quote compatibility

Imported CUE FILE delimiters are converted from single to double quotes in
staging, including on re-import of a same-size legacy copy. This addresses
DuckStation's double-quote-only parser as well as Marquee's scanner. Names,
directives and line endings are preserved; malformed/ambiguous quoting refuses
the import before publication. The source archive is unchanged.

A fresh disposable-library XS Moto RAR retest extracted all ten members and
normalized every FILE delimiter, but still reported a missing data track: the
CUE names `XS Moto (axekin.com) (En,Fr,De,Es,It) (Track 1).bin`, while the archive
contains `XS Moto   (Track 1) (axekin.com).bin` (and similarly for the audio
tracks). This independent provider packaging defect remains a diagnostic;
Marquee does not silently guess track replacements. Game launch remains untested.
