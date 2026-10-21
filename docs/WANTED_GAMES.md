# Games without files (wishlist and "owned elsewhere")

Library → **Add games → Add a game without files…**: type a title, pick the system and a status (**Wishlist**, or **I own it** when the files are elsewhere). The game appears in the Library with a "Wishlist" or "No files" badge and a disabled **Play · no files yet** button. Code: `crates/core/src/wanted.rs`, table `wanted`.

## How it works

- A wanted game is an ordinary edition with no media, so favourites, rename, search, covers and facts work unchanged. A typed title is stored as the user's own title, so "remove missing games" keeps it.
- **Cover and facts by name.** For cartridge systems the typed title is matched to the official libretro No-Intro list (USA release preferred, then Europe, then World, never homebrew over retail), which gives the official name for the libretro cover and the checksum for genre, year, developer and publisher. Descriptions and player counts come from TheGamesDB (see [ROM_FACTS](ROM_FACTS.md)); disc systems use the title as typed. The same name-match safeguards as everywhere else apply: no match means no cover, never a wrong one.
- **Files attach themselves.** When a scan or an [archive intake](INTAKE.md) brings in a file for the same system whose name is the same game (`Super Mario Bros. (USA).nes` for "Super Mario Bros."), it is attached to that entry, the wanted status is removed, and Play enables. No duplicate entry is created; your typed title is kept.
- **Library filter** "Without files" shows wishlist entries and games whose files are currently missing.
- Game page: **I own this / Move to wishlist**, **Remove** (only for entries without files; a game with files is never removed from here), **Fetch cover**.

## Limits

- Adding is manual and one game at a time; there is no import of a list yet.
- Matching a file is by name only (not checksum), so a badly named file will not attach; it appears as a separate game and the wanted entry can be removed.
- Games with several discs share a name, so only the first disc attaches.
- API: `add_wanted_game`, `set_wanted_status`, `remove_wanted_game`; no CLI command yet.
