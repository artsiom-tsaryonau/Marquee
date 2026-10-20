# Follow-up ticket: PS2 metadata matching and cached artwork

Prepared: 2026-10-20. Repository: `/home/atsaryonau/Projects/Marquee`.

Serial-based box art from xlenore/ps2-covers is **in the tree** (schema v2 `artwork` table, cached JPEG, Tauri asset protocol). See [PS2.md](PS2.md). This ticket is the remaining ScreenScraper match + SteamGridDB pipeline.

Do **not** treat xlenore JPEGs as ScreenScraper identity. The launch/exit gate is still the playtime proof; cover fetch is optional and nonfatal. **Do not start this ticket until that gate is closed**, unless the owner reorders.

## Release boundary

The first PS2 milestone is:

> A persistent, offline PS2 library that launches through an existing PCSX2 installation, observes its exit, and records actual session duration.

This ticket adds matching, user corrections and cached artwork on top of that library. Import, Play and session accounting must keep working with no network and no provider credentials.

## Why it is separate

ScreenScraper requires forum-issued developer credentials (`devid` / `devpassword`) plus optional user `ssid` / `sspassword`. It is not a SteamGridDB-style single API key. The API may only be used in fully free, publicly distributed applications unless ScreenScraper grants other terms,. Live matching therefore has product and credential prerequisites the offline slice does not.

Serial extraction and content hashing also serve different jobs. The first milestone already reads `SYSTEM.CNF` for a boot serial. Whole-ISO hashes, if added here, are for integrity, verified relocation and provider lookup — not for showing the first library card.

## Required

- Provider interface, schema and UI that never overwrite local media identity, playtime or user title/favourite corrections.
- ScreenScraper matching against documented APIs only (no HTML scraping).
- SteamGridDB artwork after a resolved title or a supported external platform ID exists.
- Optional credentials in the system keyring or explicit development-only environment configuration. Never commit secrets. Do not treat a confidential shared secret compiled into a public desktop binary as protected.
- Cached offline reuse of the last valid match/artwork. Missing credentials, downtime and no-match are nonfatal.
- Tests with fixture HTTP, not live paid or quota-limited calls, for routine automation.

## Explicitly deferred again

- Other metadata sources (IGDB, RAWG, RetroAchievements, libretro-database).
- Hash-based auto-relink as a silent merge of distinct files.
- Advertising CHD/CUE import. That remains a media-format ticket.

## Schema additions

`artwork` already exists (schema v2) for serial covers: `(edition_id, kind)` → path, source (`xlenore`), fetched_at. **Extend that table** (attribution, local-override flag, provider id). Do not create a second artwork store.

Add only when this ticket starts:

| Entity | Minimum responsibility |
| --- | --- |
| Provider match | Local edition ID, provider ID, external ID, match confidence/state, fetched timestamp and user override. |
| Artwork | Edition ID, kind, cached path, source attribution, fetched time and local-override flag (extend v2). |

Keep user titles, favourites and local artwork overrides separate from refreshed provider values.

## ScreenScraper

1. Use the documented API (`jeuInfos.php`, `jeuRecherche.php`, `ssuserInfos.php`), not HTML scraping. Recheck [webapi2.php](https://www.screenscraper.fr/webapi2.php) before coding; v2 is marked unstable.
2. Developer identity (`devid`, `devpassword`, `softname`) authorizes the application. User `ssid` / `sspassword` affect quota and member entitlements. Do not assume a SteamGridDB-style single key covers both.
3. Register the application through ScreenScraper's forum before shipping live calls. Confirm the MIT license satisfies “entirely free and distributed,” or obtain their written terms.
4. Match in this order: serial from `SYSTEM.CNF` (`serialnum`) with `systemeid=58` (PlayStation 2), then documented hash/size fields if a hash was computed for that file, then title + platform + region search. Do not hash every large ISO as a prerequisite to showing or launching a game.
5. Auto-accept only unambiguous high-confidence matches. Otherwise provide a small candidate chooser and “Keep local title.”
6. “Wrong game?” rematches without replacing local media identity or losing playtime/favourites.
7. Bounded concurrency, current quotas, timeouts, cancellation and backoff for 429/temporary failures. Cache successes and unsuccessful lookups for a bounded interval.
8. Validate user credentials on an account endpoint (`ssuserInfos.php`). Scraping endpoints may succeed on developer credentials even when the user password is wrong.

**Gate:** successful, ambiguous, unmatched, unauthorized, rate-limited and offline responses have tested behavior; metadata updates never overwrite explicit user corrections; Play still works with providers disabled.

## SteamGridDB

SteamGridDB is not name-search only. [API v2](https://www.steamgriddb.com/api/v2) supports:

- Bearer API key authentication.
- Game lookup by SteamGridDB id (`/games/id/{gameId}`).
- Lookup by supported external platform ids (`/games/{platform}/{platformId}`), including steam, origin, egs, bnet, uplay, flashpoint and eshop.
- Autocomplete search by term (`/search/autocomplete/{term}`).
- Grid/hero/logo/icon lists keyed by the resolved SteamGridDB game id.

For imported PS2 games:

1. A resolved display title (user correction or ScreenScraper match) is the practical starting point.
2. A PS2 serial is **not** a SteamGridDB platform identifier. Do not send `SLUS_209.63` as `/games/{platform}/{id}`.
3. If a later match yields a supported platform id (for example a Steam re-release), use that id rather than reconstructing identity from a filename.
4. Retain SteamGridDB game/artwork ids once resolved. Store author attribution where the API supplies it.
5. Covers first, then hero/logo. The app remains usable with only a cover or with procedural placeholders.
6. Download in the backend with bounded size/timeouts; verify image content; enforce decode/dimension limits; write atomically into Marquee's cache. Serve through a scoped Tauri asset path, not generic home-directory access.
7. Keep real artwork separate from procedural `Art` hue/motif fields. A manual local override survives subsequent refreshes.

**Gate:** matched images display in the desktop WebView and survive restart/offline use; provider failures retain placeholders/cached images and do not block launching.

## Hashing, if added here

Full-file hashes remain optional and lazy:

- Use them for integrity checks, verified relocation/deduplication and provider lookups that actually require a hash.
- Store algorithm, file format, size/mtime and hash scope. A raw ISO hash is not another container format's content identity.
- Do not block first paint, serial extraction or Play on a completed DVD-sized hash.

## Suggested commands

Extend the existing CLI only if the same core operations are already used by Tauri, for example metadata candidate list/accept and artwork refresh. Keep `--data-dir` isolation. Do not scrape the user's real keyring in tests.

Add structured errors such as `PROVIDER_UNCONFIGURED` and `RATE_LIMITED` here, not in the offline ticket.

## Automated tests

Fixture-based success, ambiguity, no match, 401/403, 429/backoff, timeout, malformed body, cancellation and cached offline response for ScreenScraper. Artwork: cache hit, atomic replacement, failed/truncated image, oversized input, unsafe path, local override preservation. UI: candidate chooser, configured/not-configured key status without echoing the secret.

## Definition of done

- [ ] Offline import, launch and playtime still work with providers disabled or unreachable. *(Serial covers already skip/404 without blocking Play.)*
- [ ] ScreenScraper matching exists with documented credentials, caching, attribution and failure handling.
- [ ] SteamGridDB artwork exists; PS2 serials are not treated as SteamGridDB platform ids.
- [ ] User title/favourite/artwork overrides survive refresh.
- [ ] LICENSE / ScreenScraper registration status is recorded; live access is not claimed if those are pending.
- [ ] README and verification docs describe the optional enrichment without implying it is required to play.
