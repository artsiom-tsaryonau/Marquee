# Cover art for cartridge and disc games

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

- Descriptions, genre, release year and developer: the libretro `metadat/` lists have them keyed by the same checksums, but the UI has no field for them yet.
- Disc images are not hashed (too large); PS1/PS2 use the serial, Dreamcast and others only the title.
- Genesis and Neo Geo arcade sets are not looked up (no emulator for the first, ZIP set names for the second).
- IGDB and TheGamesDB are not used yet. A key for TheGamesDB is in `.env` but nothing reads it.
