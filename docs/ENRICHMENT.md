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
| Emulation metadata | ScreenScraper only after its provider requirements are met; [detailed follow-up](#ps2-metadata-matching-and-cached-artwork) |
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

## PS2 metadata matching and cached artwork

Serial-based box art from xlenore/ps2-covers is already in the tree (schema v2 `artwork` table, cached JPEG, Tauri asset protocol; see [PS2.md](PS2.md)). This section covers the remaining ScreenScraper match + SteamGridDB pipeline. xlenore JPEGs are not ScreenScraper identity. **Do not start until the PS2 launch/exit/playtime gate is closed**, unless reordered. Import, Play and session accounting must keep working offline with no credentials; missing credentials, downtime and no-match are nonfatal.

**Required**

- Provider interface, schema and UI that never overwrite local media identity, playtime, or user title/favourite/artwork corrections.
- ScreenScraper through documented APIs only (no HTML scraping); SteamGridDB artwork after a resolved title or supported external platform ID.
- Credentials in the system keyring or dev-only environment config; never commit secrets, and don't treat a secret compiled into a public binary as protected.
- Cached offline reuse of the last valid match/artwork; fixture-HTTP tests, not live quota-limited calls.
- Extend the existing `artwork` table (attribution, local-override flag, provider id) rather than adding a second store. Add a provider-match entity (edition ID, provider/external ID, confidence/state, fetched time, user override) only when work starts.

**Deferred:** IGDB, RAWG, RetroAchievements, libretro-database; hash-based auto-relink as a silent merge; CHD/CUE advertising (a media-format ticket).

**ScreenScraper**

1. Use `jeuInfos.php`, `jeuRecherche.php`, `ssuserInfos.php`; recheck [webapi2.php](https://www.screenscraper.fr/webapi2.php) first (v2 is marked unstable).
2. It needs forum-issued developer credentials (`devid`, `devpassword`, `softname`); user `ssid`/`sspassword` affect quota. It is not a single-key API. Register the app first and confirm the MIT license satisfies "entirely free and distributed", or get written terms.
3. Match by `SYSTEM.CNF` serial with `systemeid=58`, then hash/size if a hash exists, then title + platform + region. Never hash large ISOs as a prerequisite to showing or launching.
4. Auto-accept only unambiguous high-confidence matches; otherwise a small candidate chooser plus "Keep local title". "Wrong game?" rematches without touching media identity or history.
5. Bounded concurrency, timeouts, cancellation, backoff for 429; cache successes and misses for a bounded interval. Validate user credentials on `ssuserInfos.php`, since scraping endpoints can succeed with a wrong user password.

*Gate:* success, ambiguous, unmatched, unauthorized, rate-limited and offline responses have tested behavior; user corrections survive; Play works with providers disabled.

**SteamGridDB** ([API v2](https://www.steamgriddb.com/api/v2)): bearer key; lookup by SteamGridDB id, by external platform id (`/games/{platform}/{platformId}`: steam, origin, egs, bnet, uplay, flashpoint, eshop), autocomplete search, grid/hero/logo/icon lists by game id.

1. For PS2, start from a resolved display title (user correction or ScreenScraper match). A PS2 serial like `SLUS_209.63` is **not** a SteamGridDB platform id.
2. If a match yields a supported platform id (e.g. a Steam re-release), use it. Keep resolved game/artwork ids and author attribution.
3. Covers first, then hero/logo; the app works with only a cover or procedural placeholders.
4. Download in the backend with size/timeout limits, verify image content, enforce decode/dimension limits, write atomically into the cache, serve via a scoped Tauri asset path. Keep real artwork separate from procedural `Art` fields; local overrides survive refresh.

*Gate:* images display in the WebView and survive restart/offline; provider failures keep placeholders and never block launching.

**Hashing, if added:** optional and lazy, for integrity, verified relocation/dedup and hash-only lookups. Store algorithm, format, size/mtime and scope; never block first paint, serial extraction or Play.

**CLI/errors/tests:** extend the CLI only for operations Tauri also uses (candidate list/accept, artwork refresh), keep `--data-dir` isolation, add `PROVIDER_UNCONFIGURED` and `RATE_LIMITED` here. Test ScreenScraper success, ambiguity, no match, 401/403, 429/backoff, timeout, malformed body, cancellation and cached offline response; artwork cache hit, atomic replace, truncated/oversized image, unsafe path, override preservation; UI candidate chooser and key status without echoing the secret.

**Done when:** offline import/launch/playtime work with providers off; ScreenScraper matching exists with credentials, caching, attribution and failures; SteamGridDB artwork exists without serial-as-platform-id; overrides survive refresh; license/registration status is recorded (no live-access claim while pending); README says enrichment is optional.
