# Artwork and facts for cartridge and disc games

`Fetch covers` (and the automatic fetch after a scan) fills in covers for games that have none. Code: `crates/core/src/romart.rs`, called from `Core::refresh_artwork`. It never replaces a cover that exists and never blocks Play.

## Order

1. **PS1/PS2 serial covers** (xlenore sets) — unchanged, see [PS2](PS2.md).
2. **libretro-database + libretro-thumbnails** for NES, SNES, Game Boy, Game Boy Color, PC Engine, N64 and DS. The ROM is hashed (CRC-32, also without an iNES or SNES copier header), looked up in the No-Intro list to get the **official game name**, and box art (then title screen, then screenshot) is fetched by that name. A messy file name does not matter. No account or key.
3. **SteamGridDB** (needs `STEAM_GRID_DB_API_KEY` in Settings → API keys or `.env`) for games that still have no cover, but **only** when:
   - the game has an official No-Intro name that looks retail (not `World`, `Proto`, `Beta`, `Demo`, `Homebrew`, `Aftermarket`, `Unl`, `Pirate`, `Hack`), or
   - it is a PS1, PS2 or Dreamcast disc image.
   A search result is used only if its name is essentially the same game (`Tetris` does not match `Tetris Effect`).

Homebrew, public-domain and hacked ROMs are deliberately left without a cover: they share names with famous games and a wrong cover is worse than none.

## Caching and speed

Lists are cached for 30 days under the cache folder (`libretro/`), ROM hashes are cached by path, size and modification time (`romcrc.json`), official names are kept (`romnames.json`), and games that found nothing are not asked again for 14 days (`romart-miss.json`); asking for one game by name bypasses that. Lookups run six at a time. On a 780-game library: first run about 18 s, repeat run under a second.

## Results on the current library (a copy of the real database)

The library is mostly public-domain and homebrew Game Boy Color and NES ROMs, so very few covers exist for it anywhere: 1 from libretro (Pokémon Platino, matched by checksum), 3 from SteamGridDB, none wrong that I could see. This will matter when commercial games are added (for example through [archive intake](INTAKE.md)).

## Not done

- Disc images are not hashed (too large); PS1/PS2 use the serial, Dreamcast and others only the title.
- Genesis now has a launch adapter; Genesis and Neo Geo remain absent from the current artwork source list. Emulator support and enrichment support are separate.
- IGDB is not used by this path. TheGamesDB is used for optional facts as described below.


## Game facts

See also the [game-facts implementation notes](ROM_FACTS.md).

`Core::refresh_artwork` also runs `romfacts.rs`. libretro checksum-keyed
metadata fills missing genre, release year, developer and publisher. A configured
`THE_GAMES_DB_API_KEY` (Settings → API keys → TheGamesDB) enables descriptions,
player counts and missing fields. Requests use a system ID and a conservatively
matched name: an official retail No-Intro name for cartridges, or the local
title for supported discs. This is enrichment, not verified release identity.
Missing or unmatched results leave facts unknown. Same-name release ambiguity
is not resolved by this search and remains follow-up work.

Facts are stored separately in `edition_info`, projected as `Edition.info` and
shown on the game page. Fields already present are retained when filling gaps;
there is no general user-edit/override model yet. The combined `source` label
identifies participating providers, not the source of every individual field.
The saved Vimm catalogue's per-field provenance remains separate.

The implementation caches attempts, caps TheGamesDB game lookups per run,
uses at most three workers and stops near its reported quota reserve. Current
retry/cap settings are described in [game facts](ROM_FACTS.md); they are
implementation limits, not guarantees about the provider's allowance. A
selected-game refresh bypasses the ordinary retry delay. Provider
outages or missing keys do not prevent Play. Supported systems follow the
artwork source list; adding a TheGamesDB platform ID alone does not enable a
system's enrichment. These implementation checks do not establish live provider
availability or description accuracy for every edition.
