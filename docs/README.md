# Marquee documentation

Updated 2026-10-22. Marquee is the library and launcher; the hidden
`marquee-manager` now runs local-archive preparation as a separate background
process. Torrent/browser adoption remains planned. Steam retains its
official client; Epic/GOG retain store-specific backends. Existing ready games
must remain launchable without the manager.

## Planning authority

| Document | Owns |
| --- | --- |
| [Roadmap](ROADMAP.md) | Product priority, current status and milestone summary |
| [Marquee manager](MARQUEE_MANAGER.md) | Acquisition flow, architecture, process/data ownership, M0–M6 gates, migration and recovery |
| [Console preparation](CONSOLE_PREPARATION.md) | Added Switch/PS3/PS4/Xbox/Xbox 360 collection: inventory, format/recipe work and per-platform acceptance |
| [Store implementation](STORE_INTEGRATION.md) | Current Steam inventory evidence; Steam → compatibility/Epic → GOG tasks and acceptance |
| [Unified library](UNIFIED_LIBRARY.md) | Game/edition/installation/profile identity, wishlist/without-files behavior and OpenGOAL track |
| [Connector priorities](MEDIA_CONNECTOR_PRIORITIES.md) | Provider scope and source-specific constraints |
| [Expansion catalogue](EXPANSION_CATALOGUE.md) | Future platform/port/collection ideas; not implemented capability or a competing work order |
| [Quiver integration](QUIVER_INTEGRATION.md) | Deferred reuse/integration plan |

The 2026-10-22 manager decision supersedes launcher-owned universal jobs and puts
the common preparation foundation ahead of completing direct stores. It does
not cancel Steam/Epic/GOG, introduce a second required UI, or close any live gate.
Older dates/numbered phases in feature notes are historical. Store order remains
Steam → compatibility/Epic → GOG after the manager foundation. M1/M2 preparation APIs, storage and the local process protocol are implemented;
torrent/browser migration, event replay and console recipes remain proposals.

## Current feature references

| Document | Purpose |
| --- | --- |
| [PS1](PS1.md), [PS2](PS2.md) | Existing disc adapters and independent open live acceptance gates |
| [Emulators](EMULATORS.md) | Managed tool catalogue, adapter matrix, source recipes, firmware and live evidence |
| [Archive preparation](INTAKE.md) | Implemented persistent ZIP/7z/RAR preparation, CLI/API/UI, recovery evidence and legacy intake |
| [Torrents](TORRENTS.md) | Current embedded transport, history, private/public policy and remaining limits |
| [Purple](PURPLE.md) | Narrow managed recipe and unverified game compatibility |
| [Launchers](LAUNCHERS.md) | Shortcuts, optional legacy bridge, branding implementation and source references |
| [Settings profiles](SETTINGS_PROFILES.md), [controllers](CONTROLLERS.md) | Configuration ownership, input work and acceptance |
| [UI](UI_UX.md), [desktop rendering](DESKTOP_RENDERING.md) | Current surfaces, readiness presentation and remaining visual/performance gates |
| [Catalogue metadata](CATALOGUE_METADATA.md) | Provider provenance, release identity and explicit installation links |
| [Enrichment](ENRICHMENT.md) | Implemented artwork/facts, cache/quota safeguards and future provider research; never gates Play |
| [Cleanup](CLEANUP.md) | Existing storage cleanup and preservation guidance |
| [Corrections](CORRECTIONS.md) | Dated findings and evidence retained for audits |

Use actual schema/code state when implementing migrations; v4–v6 are allocated.
The library and manager will have separate write ownership and durable receipt
handoff; see the manager plan. M1 prepares local sources only and always preserves originals; no added console
packages, firmware or keys have been installed.

## Maintenance boundaries

Keep a topic's instructions and evidence in its owning guide above; link to it
from the roadmap and README rather than copying its checklist. Source-asset
attribution stays beside assets. Dated correction history and substantial platform,
settings, controller and rendering guides remain separate because they preserve
distinct operational instructions or verification evidence. Use git history for
removed standalone notes; do not recreate redirect-only Markdown files.
