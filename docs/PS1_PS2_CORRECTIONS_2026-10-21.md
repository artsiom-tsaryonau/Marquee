# PS1 / PS2 correction pass

Implemented after the [real-library audit](PS1_PS2_AUDIT_2026-10-21.md), 2026-10-21.

## Library and setup UI

- Real disc entries retain their backend platform. The current collection maps to **9 PS1 entries and 2 PS2 entries**. Library platform categories show counts; filters, card labels, Home and Details agree on the platform.
- Runner readiness and setup actions are resolved per system: PS1 → DuckStation, PS2 → PCSX2. A missing DuckStation no longer borrows PCSX2's readiness or offers PCSX2 as its setup remedy.
- Settings separates PS1/PS2 folders, supported formats, detected runners and setup actions. The picker uses a platform-neutral title. CUE/BIN guidance reflects current support.
- Runner defaults persist independently under `selected_runner.ps1` and `selected_runner.ps2`. The legacy global setting remains a fallback only for its matching family. Profiles are family-checked too; unavailable selections fall back to an available matching runner. Existing settings/history need no schema migration.
- Native DuckStation commands and executable AppImages in Applications, `.local/bin` and Downloads are discoverable alongside Flatpak. Missing/non-executable programs and removed Flatpak apps are unavailable. Refresh emulators after installing or moving a runner.

## Media and launch corrections

- CUE manifests validate every referenced track, including audio, and reject missing/unreadable companions, invalid indices, oversized sheets and unsupported paths/modes. These entries stay in the library with a named diagnostic; Play is blocked. Restoring a track restores availability without deleting/reimporting the game.
- Relative track subfolders are resolved as written. Absolute/parent references and symlinks leaving the sheet folder are rejected rather than silently replaced with a basename. The emulator receives the original CUE; Marquee does not rewrite the game files.
- CUE identity is re-read from bounded metadata each scan, so replacing/restoring a BIN refreshes serial/region. A changed identity detaches stale artwork; optional background lookup can fetch a replacement. IDs, personal titles and favourites survive.
- The reader honors data-track `INDEX 01` frame offsets. Multi-sector ISO9660 reads skip raw-sector headers correctly; leading comment/blank lines do not stop boot-serial parsing.
- Adding a folder already registered under another system returns `ROOT_SYSTEM_CONFLICT`, preserving its original category and editions. Reclassification is not performed implicitly.
- Flatpak launches receive a **temporary read-only media grant**: the selected ISO/CHD file, or the containing folder for a CUE and its validated companions. This fixes the observed Vice City sandbox access failure without changing saved Flatpak overrides or granting the whole home directory. Paths incompatible with Flatpak's syntax produce an actionable error. A read-only shell probe inside the real PCSX2 sandbox verified the selected Vice City ISO is readable with this one-shot grant; it did not start the emulator.

## Session accounting

New known-failed process exits keep their session/error/duration record but do not increase the game's play count, aggregate playtime or last-played timestamp. Existing totals were not rewritten. A process exiting zero no longer automatically upgrades BIOS readiness to `launch_tested`; process exit cannot establish that a game booted.

This remains process supervision. A game crashing after real gameplay keeps elapsed time in the failed session record, but that failed session is excluded from successful-session totals. Recovered sessions with unavailable exit status retain the existing checkpoint/recovery policy. Full game-boot acknowledgement and crash-aware gameplay accounting remain future work.

## Verification and open gates

Regression coverage includes mixed-platform snapshots, per-system setup/defaults, missing/returned/replaced CUE tracks, artwork invalidation, nonzero data-track offsets, contained relative paths, invalid indices, raw multi-sector reads, root conflicts, executable discovery, scoped Flatpak argv and failed-launch accounting.

An isolated rescan of the actual folders preserved all **11 IDs**, produced **zero warnings / zero missing entries**, and identified Vice City, both MGS discs and Silent Hill as before. The frontend mapper produced the expected **9 PS1 / 2 PS2** categories and missing-DuckStation status. Native discovery was verified with a disposable executable fixture, without starting it. The user's real library, ROMs, emulator configuration and historical statistics were not modified by these checks.

Full Rust/frontend checks, strict lint, native desktop compilation and production frontend build passed; exact test totals are recorded in the implementation response because the shared tree also contains ongoing emulator/BIOS work. Rendered UI/controller behavior and real PS1/PS2 gameplay were not claimed from unit tests.

Restart the desktop/backend to load the Rust changes. Close the live gates after emulator/BIOS setup: retail game boot → input → exit → duration persisted once → restart → relaunch. Multi-disc grouping/switching, CHD serial extraction and verified gameplay detection remain separate capabilities. No BIOS or game data was downloaded as part of this correction pass.
