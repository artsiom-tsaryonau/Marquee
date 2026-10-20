# Marquee documentation

Updated 2026-10-21. **Marquee is the user's single game launcher.** Steam, Legendary, gogdl, umu/Proton, native ports, and emulator adapters supply execution/store capabilities. Lutris is optional legacy functionality, not required for store access or compatibility setup.

## Planning authority

1. [Roadmap](ROADMAP.md) — authoritative architecture, priorities, implemented/planned boundary, and milestone acceptance. New order: Steam → managed compatibility plus Epic → GOG.
2. [Direct store implementation handoff](STORE_INTEGRATION.md) — concrete next tasks, adapter/runtime boundaries, credentials, jobs, and test scenarios.
3. [Unified library and OpenGOAL](UNIFIED_LIBRARY.md) — game/edition/installation/profile identity and the separate native Jak track. Shared identity work is incremental; OpenGOAL does not jump ahead of the store order.
4. [Expansion catalogue](EXPANSION_CATALOGUE.md) — platform/port candidates and collection ideas; a separate expansion track.
5. [Enrichment and UI](ENRICHMENT.md) — background metadata/artwork and later visual/controller polish; never prerequisites for store Play.

The 2026-10-21 direct-store decision supersedes earlier numbered phases, “Lutris first,” “no Wine management,” and blanket “no game downloads” instructions. Owned-game store downloads are in scope. Original ROM/BIOS/firmware and port-extraction input data remain user-supplied.

## Feature and implementation notes

| Document | Purpose and limits |
| --- | --- |
| [PS1/PS2 correction pass](PS1_PS2_CORRECTIONS_2026-10-21.md) | Mixed-platform UI, CUE validation/offsets, media access, defaults and session-accounting corrections |
| [PS2](PS2.md) | PCSX2/offline-disc slice, setup, and its still-open live acceptance gate |
| [PS1](PS1.md) | DuckStation slice and schema-v4 root systems; verify its own live gate independently |
| [Managed emulators and BIOS](EMULATORS.md) | Seven managed emulator launchers and Tools UI, PS1/PS2 BIOS management, lifecycle/removal commands and per-platform integration limits |
| [Purple for Aion 2](PURPLE.md) | Managed setup/open recipe, runtime pins, retry/cancellation, and the unverified live game gate |
| [Tools and launchers](LAUNCHERS.md) | Implemented generic shortcuts and advanced configuration, not direct account/store integration |
| [Legacy Lutris bridge](AUTOMATIC_SOURCES.md) | Existing local-cache projection; deprioritized. Current automatic discovery still exists until code changes make it opt-in |
| [Correction report](CORRECTION_PASS.md) | Dated correction-pass evidence and known limits; historical test totals do not replace a fresh release check |
| [PS2 metadata follow-up](PS2_METADATA_FOLLOWUP.md) | ScreenScraper/SteamGridDB requirements, matching, and provider-specific follow-up |

Implementation notes can contain dated snapshots. They do not reorder the current roadmap, make unverified live gates complete, or imply planned store/runtime features already exist. For migrations, inspect the actual schema and use the next available version; do not reuse v4 for the proposed unified backend.

The direct-store documentation update itself changed no runtime behavior. The subsequent Purple integration adds an explicit, user-triggered setup recipe; it does not install anything at application startup or remove legacy integrations.
