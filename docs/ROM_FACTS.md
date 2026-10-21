# Genre, year, developer, publisher, players, description

Shown on the game page (About text, Genre · year, Developer, Publisher, Players) and used by library search. Filled by `Fetch covers` and by the automatic fetch after a scan. Code: `crates/core/src/romfacts.rs`; stored in the library database table `edition_info`.

## Sources

1. **libretro-database** `metadat/genre`, `releaseyear`, `developer`, `publisher`: for NES, SNES, Game Boy, Game Boy Color, PC Engine, N64 and DS. The ROM is matched by checksum, as for [covers](ROM_ARTWORK.md). No account.
2. **TheGamesDB** (key `THE_GAMES_DB_API_KEY`, Settings → API keys): descriptions and player counts for those systems and for PS1, PS2 and Dreamcast, by name; also fills a missing genre or year. Developers and publishers from TheGamesDB are not used (it returns ids that need large lists).

A field is filled once and never overwritten by a later source.

## Safeguards

- **Quota.** The free key allows about 1,000 lookups a month. Marquee asks only about games with an official retail name (or disc games), at most 60 per run, once per game, remembers it for 30 days, and stops when fewer than 25 lookups are left in the month. Games found are never asked again.
- **Same game only.** A result is used only if its title is the same game (`Underground` is not `Underground 2`, `Tetris` is not `Tetris Effect`).
- **No guesses for homebrew.** Homebrew, public-domain and hacked ROMs are not searched by name.
- The key is part of TheGamesDB's request URL, so request errors are dropped and never logged or shown.

## Not done

- PS1, PS2 and Dreamcast get genre and year only from TheGamesDB (libretro has no lists for them).
- Developers and publishers for disc games.
- Editing facts by hand.
