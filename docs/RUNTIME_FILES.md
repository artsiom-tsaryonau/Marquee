# Runtime files, BIOS locations and cleanup

This describes the current Fedora checkout deployment, audited 2026-10-25.
`<data>` normally means `~/.local/share/marquee`, `<config>` means
`~/.config/marquee`, and `<cache>` means `~/.cache/marquee`. An explicit
`--data-dir ROOT` instead uses `ROOT`, `ROOT/config` and `ROOT/cache`. Do not
mix those two cache/config layouts when inspecting a library.

## What is needed to run

| Files or directory | Why keep it |
| --- | --- |
| `target/release/marquee-desktop` | Built native application, with the production UI embedded. |
| `target/release/marquee-manager` | Worker companion; keep beside the desktop/CLI executable. Preparation, browser downloads and managed jobs use it. |
| `target/release/marquee` | Optional CLI and web server. Needed for the CLI commands below, not for opening the native app. |
| `<data>/library.db` and any SQLite `-wal` / `-shm` files | Library, game IDs, metadata links, titles, favorites, play history, music links and settings. Never delete live database sidecars. |
| `<data>/manager/` | Durable jobs, receipts, verified installations and firmware setup state. It is not a general temporary folder. |
| `<data>/runners/<emulator>/current` | Managed emulator or native-engine program. Keep the whole installed build, including its adjacent libraries/resources and receipt. |
| `<data>/<emulator>/` | Working BIOS/firmware, settings, memory cards, native saves, save states and caches. Do not delete the whole profile to clear a cache. |
| Registered game paths and `<data>/manager/installations/` | Actual playable game files. A library database entry does not contain the game. |
| `<config>/` | Provider keys and other local configuration. Back up privately; never publish it. |
| `<cache>/art/` and `<data>/cache/art/` | Existing local cover images. Both locations can be referenced by this library. Keep for offline artwork. |
| `<data>/music/` and `<data>/game-media/` | Imported audio, images and prepared videos. The Media tab references these files. |
| Streaming pilot storage and container images | Required for the currently configured private-display host and managed Moonlight viewer; see below. |

The native app can be opened directly from the checkout:

```bash
./target/release/marquee-desktop
```

The machine still needs its system runtime libraries (including WebKitGTK/GTK),
graphics drivers, and any dependencies of the selected emulator. The build
toolchain is separate: Rust/Cargo, Node/pnpm, `node_modules`, development headers
and debug/test outputs are needed for development or rebuilding. The convenient
`scripts/run.sh desktop` launcher checks the build using Cargo and Node; direct
execution above avoids that build check. Keep the checkout for rebuilding and
source-build workflows. This guide is not a validated portable binary bundle.

The web surface additionally needs the built `apps/desktop/dist` assets. Keep
them if using `marquee run web`; rebuilding them needs Node/pnpm. Opening the
native app does not require a Vite server.

## Where this collection lives

| Purpose | Current physical location |
| --- | --- |
| Retained original inputs | `~/Games/Marquee/sources/imported/<platform>/` |
| Imported ROM collection | `~/Games/Marquee/library/imported-roms/<platform>/` |
| Manager-prepared installations | `~/Games/Marquee/library/managed-installations/<receipt ID>/`; `<data>/manager/installations` remains a required compatibility link. |
| Other existing game files | Some registered games still use `<data>/roms/` or pilot directories; the database paths remain authoritative. |
| Relocation inventory and recovery | `~/Games/Marquee/.storage/migration.json` |

The checkout's `sample_images` and `roms` are compatibility links. Keep them:
existing library paths, receipts and launch manifests can still use them.
They do not represent extra payload copies. Do not remove other pilot folders
based on their names: some contain the only game data, provenance or configured
runtime for a feature. See [storage migration](STORAGE_LAYOUT.md).
The manager's installation location record and recovery journals under
`<data>/manager/installation-moves/` are also required. Use
[Storage's reviewed move controls](STORAGE_MANAGER.md#managed-installations)
instead of renaming installation folders manually.

For an individual game, keep the whole format's dependency set:

- Cartridge ROM: the registered ROM file, plus the emulator and its save profile.
- CUE disc: the CUE **and every track it references**. Keeping only the small CUE
  file does not keep the game. Keep multi-disc sets and playlist targets together.
- Verified ISO/CHD launch: the registered image plus the selected emulator,
  required working firmware and user profile. PS3 and Xbox 360 currently use
  their explicitly validated ISO paths; a package is not interchangeable with one.
- Native port/decomp: the installed engine, adjacent resources and its compatible
  original/generated game assets (such as Shipwright's O2R), plus user data.
  A renamed game folder or a copied executable alone is not a portable runtime.
- Archive/torrent: retained source or transport payload. It may still need
  preparation before launch; extraction does not make every archive member safe
  to delete. Follow its manager receipt rather than choosing files by extension.

## BIOS and firmware are dependencies, not games

BIOS files normally live in emulator profiles, **not in the Games collection**.
Original firmware packages and installed firmware are different things: keeping
a PS3 update package does not itself install firmware into RPCS3.

| System | Marquee location and current behavior |
| --- | --- |
| BIOS import inbox | `<data>/bios-inbox/`. A folder saved in the `bios_inbox` setting is an additional import source. Recognized inputs are copied to emulator destinations during launch; dropping a file here does not prove compatibility. |
| PS1 / DuckStation | `<data>/duckstation/xdg-config/duckstation/bios/` |
| PS2 / PCSX2 | `<data>/pcsx2/xdg-config/PCSX2/bios/`. Keep recognized BIOS images and their sidecars together. |
| PS3 / RPCS3 installed firmware | `<data>/rpcs3/xdg-config/rpcs3/dev_flash/`. Keep the whole RPCS3 profile: its other directories include installed content and user data. |
| PS3 update packages | Managed setup retains staged `PS3UPDAT.PUP` files under `<data>/manager/ps3-firmware/work/<job ID>/`. The catalogue inbox destination, when used, is `<config>/rpcs3/firmware/`; it is not the installed firmware tree. |
| Dreamcast / Flycast | Optional `dc_boot.bin` under `<data>/flycast/`. The installed adapter also supports its HLE boot path. |
| Original Xbox / xemu | Descriptor import destination is `<data>/xemu/xemu/` for MCPX, flash BIOS and HDD image. This machine has no configured xemu profile yet; file presence alone would not finish setup. A working HDD contains mutable user data and must not be treated as a disposable BIOS file. |
| Neo Geo / FBNeo | **Current exception:** matching `neogeo.zip` beside the game ZIP, plus parent sets where required. The scanner excludes `neogeo.zip` from Games. Do not unpack or rename sets to organize them. |
| DS / melonDS | The current direct-boot adapter uses built-in BIOS replacements; DSi firmware boot is not configured. |

The audit found populated PS1 and PS2 BIOS folders and RPCS3's installed
`dev_flash`. The BIOS inbox also contains a retained input. Keep these when
cleaning game archives. A versioned original-firmware vault under
`sources/firmware/` remains planned; installed profiles have not been relocated
into `~/Games/Marquee`.

## Streaming files to keep

The main library currently selects `<checkout>/data/streaming-pilot` as its
streaming data root. Keep its `streaming/wolf/state` (pairing/configuration),
managed receiver files, runtime/image receipts and
`streaming/library/profiles/<library hash>/<adapter>/<edition ID>` (streamed
game profiles and saves). Keep the referenced Podman images, too. These paths
are live dependencies even though their parent directory says `pilot`.

Do not remove a container/image merely because it is stopped. Closing Moonlight
can leave the game running, and an image receipt is needed for future launches.
See [streaming ownership](STREAMING.md) before changing this storage.

## Metadata and covers for existing games

Every imported game already has a local title, platform and file record; some
also have a serial/region or linked catalogue entry. Full descriptions, release
facts and cover images require an available provider match. Those records live
in `library.db`; cover files are cached separately, rather than being mandatory
files beside each ROM. Music album artwork may also be stored in the database.

The 2026-10-25 refresh increased covers from **7 to 14** and fact records from
**6 to 22**, across **858 games**. It added covers for Need for Speed Underground,
Pokémon Platinum, Streets of Rage 2 and several smaller titles. All saved cover
paths resolve. Game IDs, favorites, play history, roots, media and preparation
receipts were unchanged. A consistent pre-refresh SQLite backup was retained.

Many supplied games are homebrew, demos or lack an accepted artwork match.
Automatic lookup uses supported ROM checksum lists, disc serials and configured
providers; it leaves unknown or ambiguous identities alone. An official-name
match does not guarantee that a provider has cover art or detailed facts.
Placeholder artwork is therefore expected for part of this collection.

To update existing games, use **Fetch covers**, or close the app sharing this
library and run:

```bash
./target/release/marquee artwork refresh
# Retry one title without the ordinary missing-result backoff:
./target/release/marquee artwork refresh --edition EDITION_ID
```

On a game's **Metadata** tab, search artwork and game details, review the result
and apply the correct match. SteamGridDB and TheGamesDB searches use their keys
from **Settings → API keys**. Search results are separate from verified release
identity; do not select a similarly named retail game for an unrelated homebrew.
Existing covers and filled facts are retained by the automatic gap-filling path.
See [enrichment sources and limits](ENRICHMENT.md) and
[catalogue matching](CATALOGUE_METADATA.md).

## Safe cleanup and what to preserve

With development builds/tests stopped, Rust debug/test build output can be
removed without affecting the release app or collection:

```bash
cargo clean --profile dev
```

This reclaims build space; the next `scripts/check.sh`, development run or web
launcher can regenerate it. Python `__pycache__` output from local experiments
can also be regenerated. The cleanup performed with this guide removes those
generated files, keeping release executables and all collection/profile data.
The 2026-10-25 cleanup removed 96,080 build-output files (Cargo reported
71.1 GiB total) and one experiment bytecode cache. Afterward the release CLI
reported a healthy 858-game library, all 858 media paths and all 14 covers were
available, and the three release executables remained present. The full check
suite and production build passed before cleaning their development outputs.

Keep game inputs, original archives, BIOS/firmware, licenses, save data, receipts,
database backups and cached artwork. Archive deletion still needs a recovery
and dependency decision: installation success alone does not prove a source is
redundant, and a torrent needs its exact source payload to seed. No source
archive, installation, firmware or save is deleted by this cleanup.

For backup, use SQLite's backup API or stop the application before taking a
consistent database copy. Back up the referenced media, profiles and artwork
alongside it. Copying only a live `library.db` can omit WAL changes; copying
only that database also does not include the games or cover files.
