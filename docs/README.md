# Marquee documentation

Updated 2026-10-21. **Marquee is the user's single game launcher.** Steam, Legendary, gogdl, umu/Proton, native ports, and emulator adapters supply execution/store capabilities. Lutris is optional legacy functionality, not required for store access or compatibility setup.

## Planning authority

1. [Roadmap](ROADMAP.md) — authoritative architecture, priorities, implemented/planned boundary, and milestone acceptance. New order: Steam → managed compatibility plus Epic → GOG.
2. [Direct store implementation handoff](STORE_INTEGRATION.md) — concrete next tasks, adapter/runtime boundaries, credentials, jobs, and test scenarios.
3. [Unified library and OpenGOAL](UNIFIED_LIBRARY.md) — game/edition/installation/profile identity and the separate native Jak track. Shared identity work is incremental; OpenGOAL does not jump ahead of the store order.
4. [Expansion catalogue](EXPANSION_CATALOGUE.md) — platform/port candidates and collection ideas; a separate expansion track.
5. [Enrichment and UI](ENRICHMENT.md) — background metadata/artwork and later visual/controller polish; never prerequisites for store Play.
6. [Media acquisition](MEDIA_ACQUISITION.md) — automated firmware/disc/port data jobs; manual import as fallback.

The 2026-10-21 direct-store decision supersedes earlier numbered phases, “Lutris first,” “no Wine management,” and blanket “no game downloads” instructions. Owned-game store downloads are in scope. The same job model should cover firmware, disc images, and port inputs from **authorized sources** (stores, detected installs, configured connectors) — not manual-only workflows. See [media acquisition](MEDIA_ACQUISITION.md). Marquee does not bundle copyrighted BIOS/ROM files in the app.

## Feature and implementation notes

| Document | Purpose and limits |
| --- | --- |
| [PS2](PS2.md) | PCSX2/offline-disc slice, setup, and its still-open live acceptance gate |
| [PS1](PS1.md) | DuckStation slice and schema-v4 root systems; verify its own live gate independently |
| [Media acquisition](MEDIA_ACQUISITION.md) | Target: automated firmware/disc/port jobs from authorized sources; manual import as fallback |
| [Media connector priorities](MEDIA_CONNECTOR_PRIORITIES.md) | FMHY-informed tiers: Vimm/Myrient/DAT verification vs do-not-automate ROM hosts |
| [Managed emulators and BIOS](EMULATORS.md) | Seven managed emulator launchers and Tools UI, BIOS management, game-only window research, and the mGBA source-build pilot |
| [Purple for Aion 2](PURPLE.md) | Managed setup/open recipe, runtime pins, retry/cancellation, and the unverified live game gate |
| [Tools, launchers and legacy Lutris bridge](LAUNCHERS.md) | Generic shortcuts and advanced configuration; the deprioritized Lutris cache bridge (automatic discovery stays until made opt-in) |
| [Launcher branding](LAUNCHER_BRANDING.md) | Official marks, Simple Icons slugs, and UI wiring notes for Steam/Epic/GOG/Lutris/Heroic/Purple/runtimes |
| [Desktop rendering](DESKTOP_RENDERING.md) | Production launch, renderer modes, Balanced/Full effects, and remaining acceptance |
| [Enrichment, UI and PS2 metadata](ENRICHMENT.md) | Optional metadata/artwork providers and the ScreenScraper/SteamGridDB follow-up |
| [Catalogue metadata](CATALOGUE_METADATA.md) | Saved-page import, regional revisions/languages, source artwork and ratings, explicit installation links and raw-ISO SHA-1 matching; schema v5 |
| [Library transfer (BitTorrent)](TORRENTS.md) | Optional private device-to-device transfer: status, privacy defaults, CLI, next steps |
| [Archive intake](INTAKE.md) | `marquee intake`: unpack `.zip`/`.7z` games into the library; rules, safety limits and gaps |
| [Cleanup](CLEANUP.md) | What can be deleted (build output, backups, quarantined files), what it costs, and what to keep |
| [Correction history](CORRECTIONS.md) | Condensed 2026-10-20/21 corrections, PS1/PS2 audit outcomes and open gates |

Implementation notes can contain dated snapshots. They do not reorder the current roadmap, make unverified live gates complete, or imply planned store/runtime features already exist. For migrations, inspect the actual schema and use the next available version; v4 and v5 are allocated and must not be reused for the proposed normalized unified backend.

The direct-store documentation update itself changed no runtime behavior. The subsequent Purple integration adds an explicit, user-triggered setup recipe; it does not install anything at application startup or remove legacy integrations.
