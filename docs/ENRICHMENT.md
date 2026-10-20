# Enrichment and UI follow-up

Planning notes aligned 2026-10-21. These retain metadata, visual, and controller goals from the earlier roadmap. They are background/follow-up work; [direct Steam → Epic → GOG](ROADMAP.md) is the critical path. Nothing here gates a supported installed game's Play action.

## Metadata and artwork candidates

| Area | Planned approach |
| --- | --- |
| Steam artwork | App-ID based artwork lookup, cache and fallback; verify current asset locations during implementation |
| Disc covers | Retain existing system/serial cover mapping; provider misses keep procedural art |
| General cover/hero/logo/icon | SteamGridDB after a verified external ID or title match; preserve source/author/asset provenance and user overrides |
| Canonical game facts | IGDB matching and supported cross-store IDs; RAWG as an optional fallback after verifying access and attribution |
| Reviews and ratings | Steam review summaries; named sources with fetch dates, counts, and links. Do not combine incompatible scores into a fabricated rating |
| Achievements | Verified Steam APIs and optional RetroAchievements integration; ownership/authentication and runtime achievement capabilities are separate |
| Save locations and fixes | PCGamingWiki data and explicit tested recipes; a metadata suggestion is not proof a fix was applied |
| Compatibility | Optional ProtonDB/community and anti-cheat information, versioned and credited; actual tested game/runtime evidence takes precedence |
| Emulation metadata | ScreenScraper only after its provider requirements are met; [detailed follow-up](PS2_METADATA_FOLLOWUP.md) |
| Additional providers | OpenCritic/MobyGames or similar only after current API access, attribution, and distribution requirements are verified |
| Deep-dive sites | Link-outs to SteamDB, Metacritic, and HowLongToBeat; no general web-scraping dependency |
| Optional Steam extras | News/patch notes, supported app-info/cache parsing, players-now, wishlist/prices, and small local histories; individually capability-gated and outside core store acceptance |

Use account/store/product/platform/revision identifiers before name matching. A name/year/platform match may suggest a candidate; it must not silently merge installations or editions. Provide a saved correction action. Do not hash large disc files merely to obtain a title when a supported serial-based match exists.

Cache provider results with fetch/source/version data, sensible TTLs, offline behavior, and bounded request queues. Recheck provider endpoints, quotas, access, and attribution rules at implementation time instead of inheriting old numeric assumptions. Provider keys remain backend-side; never require an artwork key to launch a game.

## Visual and controller goals

- Home uses real library entries for Continue playing, Recently added, source/platform shelves, and native-port readiness. Any backlog/score-based shelf needs actual available metadata.
- Canonical game pages show edition and installation/run choices, proper hero art, provenance-labeled screenshots, readiness, and supported Install/Play/Update/Verify/Repair actions. Preserve separate saves and histories.
- Library filters source, platform, runtime, installed state, and user tags/favorites; paginate or virtualize large collections. Do not claim a frame-rate target is achieved until measured.
- Download jobs expose real progress, cancellation, retries, and errors across desktop/browser. A client handoff is a different state from a managed install.
- Dark/light tokens, bundled type, restrained blur, reduced motion, idle animation pause, and clear focus states remain goals. Avoid stacked effects that consume the GPU while a game runs.
- Desktop and Big Screen share components; Big Screen uses larger targets, controller glyphs, and reliable back/context behavior.
- Verify a native gamepad input path such as gilrs in Tauri rather than treating browser Gamepad API tests as proof of WebView support. Disabled/inert controls stay outside spatial navigation.
- Capture and inspect actual layouts at supported sizes and input modes. A production build or reference artwork collection is not visual acceptance evidence.

## Later extras

Store-specific optional facts, collection rules, per-game notes, save backup/restore, Steam shortcut export, Gamescope launch, streaming/RomM adapters, and standalone Windows-app installer recipes remain possible after the core store flows. A standalone installer recipe may provision a prefix/runtime/dependencies and discover shortcuts, but the existing demo wizard is not an implemented installer.

Torrent/tracker integration remains deferred. Keep optional enrichments and additional stores out of S1/E1/G1 acceptance criteria.
