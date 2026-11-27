# Metadata enrichment: artwork, facts and provider plans

Updated 2026-11-27. This document owns current optional ROM artwork/facts behavior
and future provider research. [Catalogue metadata](CATALOGUE_METADATA.md) owns
saved provider snapshots and verified/manual links; [UI/UX](UI_UX.md) owns visual
and controller presentation. [Roadmap](ROADMAP.md) owns priority. Enrichment must
never gate Play, create ownership, or merge releases by name.

## Implemented artwork and facts

For the owner's current coverage audit and the on-disk locations to preserve,
see [runtime files and existing-game metadata](RUNTIME_FILES.md#metadata-and-covers-for-existing-games).
The 2026-10-25 gap-fill refresh added seven covers (14 total) and expanded fact
records from six to 22 while preserving all 858 game identities and play history.
An earlier audit found 182 cached official-name matches and 138 uncovered games
excluded solely by a blanket `(World)` rule. World is a region, not evidence of
homebrew. Current lookup accepts it when other matching safeguards hold; automatic
SteamGridDB lookup additionally requires a known, compatible release year for
World-labelled cartridge entries. Demo/homebrew/hack tags remain excluded.

### Images and details refresh (2026-11-27)

**Settings → Library locations → Refresh images and details** and the selected-game
refresh now fill missing covers and facts and download images into **Game → Media
→ Images**. Existing local covers and nonempty facts are retained. A correction
made during a network request wins over the older refresh snapshot. Game IDs,
media identity, names, favourites and play history are not rewritten by enrichment.

- PS3, Saturn and 3DS join the automatic artwork source list; TheGamesDB has
  platform mappings for Saturn and 3DS. Large disc and 3DS images are not hashed
  merely to obtain artwork.
- Filename-index fallback matches the full base title within the same platform,
  filters demos/prototypes/hacks, and retains exact revision filenames when
  available. The official per-system libretro GitHub repositories are a fallback
  when the exported thumbnail server is unavailable. Git symlink aliases are
  excluded from raw-image downloads so their text is never cached as a picture.
- SteamGridDB automatic matching requires a unique complete title, normalizes
  common accents, and checks a known release year. Ambiguous results remain for
  reviewed selection. Provider edition qualifiers are not discarded as ROM tags.
- TheGamesDB returns developer/publisher IDs; cached name lists now resolve them.
  Refresh retains the selected game ID and uses it on subsequent lookups. A known
  US region can narrow matches; otherwise duplicate regional records are not
  silently resolved. Existing descriptions no longer prevent filling other gaps.
- TheGamesDB supplies up to two screenshots, one background image and one title
  screen per game, with a missing front cover as fallback. Libretro supplies
  gameplay/title images where needed. Galleries retain public image source URLs;
  API URLs containing credentials are neither stored nor surfaced in errors.
- Downloaded galleries use stable per-edition identities and content-addressed
  image files. Refresh is idempotent, keeps local collections, and validates
  decoding/dimensions before adding images. It does not fetch videos or game data.
- Image-list requests are cached for 30 days, limited to 30 fresh lookups per run,
  and stop at the provider quota reserve. Gallery downloads have a 64 MiB run
  budget; failed URLs wait seven days unless a selected-game refresh is requested.
  Fact and cover retry caches use new versioned names for this expanded lookup.

Provider identities are references for presentation, not verified ROM/release
identity. Unknown fields remain empty. General manual fact editing, cover/hero
choice with preview/undo, author/asset provenance for older cover rows and refresh
progress/cancellation remain separate follow-ups.

Primary references: [TheGamesDB API](https://api.thegamesdb.net/),
[libretro thumbnail repository](https://github.com/libretro-thumbnails/libretro-thumbnails),
[SteamGridDB API v2](https://www.steamgriddb.com/api/v2).

### Follow-up metadata and media audit (2026-11-27)

Scope: source review and a local headless browser against an isolated copy of the
18-game, 50-image library, with provider keys disabled. Screenshots and fault
injection used the copy only. No game launch, native gamepad, torrent transfer or
new live-provider acceptance is implied by this pass.

**Fixed:** choosing a different reviewed TheGamesDB release could leave the first
provider ID in the provenance string controlling subsequent refreshes. The newest
reviewed ID now wins, including reselection of a previous ID. Historical source
labels remain. Facts, provider covers and gallery inserts check that the reference
is still current after network work, so an older download cannot reapply a replaced
match. Regression tests reproduce replacement and reject stale image/cover writes.

Remaining priorities, in recommended implementation order:

| Priority | Finding and evidence | Improvement |
| --- | --- | --- |
| P2 | **One media URL failure blanks healthy previews.** Injecting one HTTP failure into the five-image Moon collection produced zero image thumbnails and five video placeholders. `MediaLibrary.refresh` waits on one `Promise.all` before saving any resolved URLs. | Resolve independently, retain successful URLs, show an image-specific error and provide per-asset retry. |
| P2 | **Image spoiler flags have no effect.** Marking a copied screenshot as containing spoilers left its thumbnail and full image visible with no warning. Only the video branch renders a badge. | Hide flagged thumbnails and require a deliberate reveal in the viewer; apply the policy during next/previous navigation too. |
| P2 | **Refresh does not remember curation.** Source review: `media_remove` deletes the asset row without an exclusion, while `save_gallery` recreates assets and inserts missing game links for the stable provider collection. Unlinked provider collections can therefore relink on refresh. | Persist hidden sources and dismissed collection links, with an explicit reset/restore action. Preserve user-added collections. |
| P2 | **Artwork replacement has no asset preview or undo.** Source review: the candidate chooser shows a title/provider ID, then the backend downloads the first compatible grid and replaces the cover. | Show available assets, dimensions and provenance; preview beside the current cover and keep a reversible previous selection. |
| P2 | **Regional matches are difficult to review and populated mistakes cannot be corrected.** A read-only two-release fixture rendered identical titles/platforms/years apart from opaque IDs. Country and full release date are discarded, and Apply only fills empty fields. | Carry region/date/edition information into the review; show field differences and allow explicit per-field correction with undo. |
| P2 | **Gallery work scales with all assets, even in Videos.** Opening global Images requested 50 media URLs while displaying two thumbnails. Switching to Videos issued another 50 requests with no video assets. Collections are also presented as 16 wrapping tabs without search. | Resolve visible assets on demand, filter before requesting, preserve URL state between views, and add collection search/grouping. |
| P2 | **Refresh offers no useful progress or cancellation.** Source review: the complete cover/fact/gallery pipeline returns one report; the UI holds a single busy flag. | Run a tracked background job with game/provider progress, cancellation and scoped retry. |
| P3 | **Controller and media-management polish remains.** Preview summaries lack the spatial-navigation marker; repeated edit/remove buttons lack asset-specific accessible names. Removed media rows leave cached files behind. Downloaded fanart has no user-facing hero-selection flow. | Add controller-reachable previews and descriptive labels; provide safe orphan-cache cleanup and a cover/hero picker. |

At 390 px the reviewed metadata fixture had no horizontal overflow. The browser
reported no JavaScript errors during these scenarios. Desktop screenshots verify
the current layout; physical controller and native fullscreen behavior remain
separate acceptance checks.


`Refresh images and details`, automatic refresh after scans, and `Core::refresh_artwork` use
`crates/core/src/romart.rs` and `romfacts.rs`. Covers fill gaps without replacing
existing covers. Missing keys, provider errors and no-match results are nonfatal.
Settings → API keys supplies configured provider keys, with environment/`.env`
fallback as documented in the root README.

### Artwork lookup order

1. **PS1/PS2 serial covers** (xlenore sets) — unchanged, see [PS2](PS2.md).
2. **libretro-database + libretro-thumbnails** for Genesis / Mega Drive, NES, SNES, Game Boy, Game Boy Color, PC Engine, N64 and DS. The ROM is hashed (CRC-32, also without an iNES or SNES copier header), looked up in the No-Intro list to get the **official game name**, and box art (then title screen, then screenshot) is fetched by that name. A messy file name does not matter. No account or key.
3. **SteamGridDB** (needs `STEAM_GRID_DB_API_KEY` in Settings → API keys or `.env`) for games that still have no cover, but **only** when:
   - the game has an official No-Intro name that looks retail (not `Proto`, `Beta`, `Demo`, `Homebrew`, `Aftermarket`, `Unl`, `Pirate`, `Hack`), or
   - it is a PS1, PS2 or Dreamcast disc image.
   A search result is used only if its name is essentially the same game (`Tetris` does not match `Tetris Effect`).

Homebrew, public-domain and hacked ROMs are deliberately left without a cover: they share names with famous games and a wrong cover is worse than none.

Genesis was missing from the shared source list despite having a TheGamesDB
platform mapping. It now uses the official `Sega - Mega Drive - Genesis` No-Intro
and thumbnail names, which also enables the existing checksum-based fact path.
This does not verify every Genesis release or manufacture a description.
[Upstream database](https://github.com/libretro/libretro-database/blob/master/metadat/no-intro/Sega%20-%20Mega%20Drive%20-%20Genesis.dat).
For thumbnail filename compatibility, lookup may omit a trailing alternate-title
annotation containing ` ~ ` while retaining every region/revision annotation.
For example, Sonic 2's `(Sonic Compilation ~ Sonic Classics)` can be omitted
after `(World) (Rev B)`; a different revision or numbered sequel is not substituted.

Music album images follow a separate bounded embedded/local artwork path, with
explicit source labels and a linked-game fallback; see [soundtracks](SOUNDTRACKS.md).
A game cover is never silently presented as a verified soundtrack release image.

Live 2026-10-24: selected-game refresh for the owner's existing Sonic 2 Genesis
entry fetched one libretro cover and filled Action, 1992, SEGA developer/publisher
facts. The downloaded PNG was visually inspected as Sonic 2 artwork. A private
database backup and source/hash receipt were retained; no description was invented
or claimed as fetched. The upstream World/Rev B thumbnail chooses its own cover
presentation; it does not certify packaging for every territory of a World ROM.

### Artwork cache and dated measurements

Lists are cached for 30 days under the cache folder (`libretro/`), ROM hashes are cached by path, size and modification time (`romcrc.json`), official names are kept (`romnames.json`), and games that found nothing are not asked again for 14 days (`romart-miss.json`); asking for one game by name bypasses that. Lookups run six at a time. Recorded on the 780-game collection at implementation time: first run about 18 s, repeat run under a second. This is a dated sample, not a current performance guarantee.

### Recorded sample result (2026-10-21)

The library is mostly public-domain and homebrew Game Boy Color and NES ROMs, so very few covers exist for it anywhere: 1 from libretro (Pokémon Platino, matched by checksum), 3 from SteamGridDB, none wrong that I could see. This will matter when commercial games are added (for example through [archive intake](INTAKE.md)).

### Artwork limits

- Disc images are not hashed (too large); PS1/PS2 use the serial, Dreamcast and others only the title.
- Genesis now has a launch adapter and a libretro artwork source. Neo Geo remains
  absent from the current artwork source list. Emulator support and enrichment
  support are separate.
- IGDB is not used by this path. TheGamesDB is used for optional facts as described below.



### Fact storage and identity limits

Facts are stored in `edition_info`, projected as `Edition.info`, and shown on
game pages. Gap filling retains existing fields. There is no general user-edit/
override model. Combined `source` labels identify participating providers, not
the evidence for each field; catalogue per-field provenance remains separate.
Name-based TheGamesDB/SteamGridDB matching does not verify a release or resolve
same-name editions. Unknown facts stay unknown. Fact lookup uses at most three
workers; selected-game refresh bypasses the ordinary retry delay. Adding a
platform ID alone does not enable a system absent from the supported source list.
These checks do not establish live provider availability or accuracy for every title.

Shown on the game page (About text, Genre · year, Developer, Publisher, Players) and used by library search. Filled by `Fetch covers` and by the automatic fetch after a scan. Code: `crates/core/src/romfacts.rs`; stored in the library database table `edition_info`.

### Fact sources

1. **libretro-database** `metadat/genre`, `releaseyear`, `developer`, `publisher`: for Genesis / Mega Drive, NES, SNES, Game Boy, Game Boy Color, PC Engine, N64 and DS. The ROM is matched by checksum, as for [covers](ENRICHMENT.md#implemented-artwork-and-facts). No account.
2. **TheGamesDB** (key `THE_GAMES_DB_API_KEY`, Settings → API keys): descriptions and player counts for those systems and for PS1, PS2 and Dreamcast, by name; also fills a missing genre or year. Developer and publisher IDs are resolved through bounded, cached provider lists.

A field is filled once and never overwritten by a later source.

### Fact safeguards

- **Quota.** The earlier implementation note described about 1,000 monthly lookups for its free key; verify current provider allowance when changing this integration. Marquee asks only about games with an official retail name (or disc games), at most 500 per run, once per game, retries a game that found nothing after 7 days, and stops when fewer than 25 lookups are left in the month. Games found are never asked again.
- **Same game only.** A result is used only if its title is the same game (`Underground` is not `Underground 2`, `Tetris` is not `Tetris Effect`).
- **No guesses for homebrew.** Homebrew, public-domain and hacked ROMs are not searched by name.
- The key is part of TheGamesDB's request URL, so request errors are dropped and never logged or shown.

### Fact limits

- PS1, PS2 and Dreamcast get genre and year only from TheGamesDB (libretro has no lists for them).
- Missing provider fields remain unknown even when a game has a reviewed match.
- Editing facts by hand.

## Remaining work

Verified cross-provider IDs, exact artwork URLs and edition associations,
general field provenance, user-edit/override precedence and ambiguity handling
remain open. ScreenScraper is research, not an active dependency. IGDB/RAWG are
not used by the implemented ROM path. Existing libretro matching and TheGamesDB
facts must remain functional independently of future provider work.

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


## PS2 metadata matching and cached artwork

Serial-based box art from xlenore/ps2-covers is already in the tree (schema v2 `artwork` table, cached JPEG, Tauri asset protocol; see [PS2.md](PS2.md)). This section covers the remaining ScreenScraper match + SteamGridDB pipeline. xlenore JPEGs are not ScreenScraper identity. **Do not start until the PS2 launch/exit/playtime gate is closed**, unless reordered. Import, Play and session accounting must keep working offline with no credentials; missing credentials, downtime and no-match are nonfatal.

**Required**

- Provider interface, schema and UI that never overwrite local media identity, playtime, or user title/favourite/artwork corrections.
- ScreenScraper through documented APIs only (no HTML scraping); SteamGridDB artwork after a resolved title or supported external platform ID.
- Credentials in the system keyring or dev-only environment config; never commit secrets, and don't treat a secret compiled into a public binary as protected.
- Cached offline reuse of the last valid match/artwork; fixture-HTTP tests, not live quota-limited calls.
- Extend the existing `artwork` table (attribution, local-override flag, provider id) rather than adding a second store. Add a provider-match entity (edition ID, provider/external ID, confidence/state, fetched time, user override) only when work starts.

**Still outside this provider proposal:** IGDB/RAWG/RetroAchievements integration, silent hash-based relinking and new media-format support. libretro enrichment is already implemented as described above; it is not deferred by this older proposal.

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

### Live refresh acceptance — 2026-11-27

The current library began with 17 entries and 10 covers. The approved live refresh
added six covers and 37 gallery images with no failed downloads. Six same-game,
platform-scoped provider references were reviewed for regional or translated-title
ambiguity; selected refreshes added nine more images and filled additional gaps.
A concurrently imported Xbox 360 Dreamcast Collection also gained a cover/details
on the repeat refresh. Final coverage: 17 covers across 18 entries, 46 downloaded
images linked to 15 games, and 16 fact records improved. The only uncovered entry
has missing Neo Geo game files. Descriptions for Dreamcast Collection and UEFA Euro 96 remain empty after
this refresh; unknown fields are not invented.

Headless verification displayed the Pokémon Moon gallery with five decoded images
and readable captions. Repeat refresh kept the total gallery count at 50 (46 new
provider images plus four existing local assets). Checks retained every original
game, media/root row, session, port session, original cover byte and existing local
media asset; existing nonempty facts were preserved. Newly imported records were
retained alongside the originals. A private pre-refresh database and receipt are
kept under the runtime data directory's `metadata-refresh-backups/2026-11-27/`.
The complete repository check script passed with two test threads.
