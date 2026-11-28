# Portable Marquee installations — proposal

Updated 2026-11-28. **Game packages work** (MP1, CLI only): export and import of
`.marquee` files. Runtime (emulator) packages, a desktop UI and network transfer
are not built. See [Using game packages](#using-game-packages).

A prepared game can be reusable across Marquee instances without converting it
into a native executable. For PS3, retain the verified disc ISO that RPCS3 already
boots directly. Repacking or extracting that ISO again is unnecessary. RPCS3
still runs the PS3 executable; its CPU/GPU-specific caches are not a portable
replacement for the game and emulator.

The portable unit should contain a versioned manifest, immutable payload file
manifest and a reviewed runtime-recipe reference. A self-contained export adds
the payload; a manifest-only export must explicitly require an existing matching
artifact at the destination. Neither form may claim to contain missing data.

The manifest should record:

- Platform, title ID, edition/version where established, base/update/DLC role and
  relationships. A title ID alone is not a complete payload identity.
- Relative payload paths, sizes and SHA-256 values; original artifact provenance,
  preparation recipe/version and output receipt identity. File hashes establish
  byte identity, not publisher authenticity or game compatibility.
- Runtime ID/version/OS/CPU and reviewed launch recipe ID. The importer resolves
  that recipe locally; manifests cannot supply shell scripts or arbitrary argv.
- Portable title-specific settings with an explicit allowlist. Display, audio
  devices, GPU choice, driver paths, absolute paths and credentials stay local.
- Optional metadata and locally owned artwork/media with their source labels.
  Saves are separate, opt-in transfers with conflict review, not default payload.

Runtime installation, PS3 firmware and local account/license/key material remain
separate requirements. Do not silently bundle a user's entire RPCS3 profile or
copy its `dev_hdd0` across libraries: that mixes installed data, saves, users and
other games. Runtime program files may be shared read-only; each instance retains
its own mutable configuration, firmware/user storage and caches. A transferred
PS3 game can still need title-specific settings or updates on the receiving host.

This extends the artifact/installation registry described in
[MARQUEE_MANAGER.md](MARQUEE_MANAGER.md), rather than adding another installation
authority. An output manifest remains useful after an optional source archive is
removed. Torrent seeding still depends on the exact original torrent payload,
not the prepared installation; package exports cannot replace that dependency.

## Using game packages

    marquee package export <edition-id> --out <folder>
    marquee package inspect <file>          # checks the manifest and file list, changes nothing
    marquee package import <file> [--into <library folder>]

`marquee list` shows edition IDs. A package is a ZIP64 with stored (uncompressed)
members: `marquee.json` (schema 2, `crates/manager/src/portable.rs`), then
`game/` and, for PS4, `update/` and `addcont/`. Code:
`crates/core/src/packages.rs`.

| Packaged | Not packaged |
| --- | --- |
| PS4 game folder, its `-UPDATE` or `-patch` folder, shadPS4 add-ons for the title | Saves, BIOS, firmware, keys, `.rap` licences |
| Single-file games (ISO, ROM, XISO, NSP and similar) | Per-title fixes and key bindings: the receiving Marquee seeds them from its catalogue |
| The exporting emulator's ID and version, as information | The emulator itself |

Not supported yet, and refused: `.cue`/`.gdi`/`.m3u` sets, Wii U folder games,
PS3 games installed in RPCS3, and extra content kept outside the game file for
other systems (Switch updates and DLC, Wii U add-ons).

Import needs a library folder for the system, checks free space, extracts into a
hidden staging folder, verifies every SHA-256, and only then moves the game into
place. It never replaces an existing file; a damaged or altered package changes
nothing. Nothing in a package is executed. Hashes prove the files match the
manifest, not who made the package: import only packages you made or trust.

Real-game check (2026-11-28): The Last of Us Remastered with update 1.11 and 7
add-ons (189 files, 63.6 GB; export 73 s, import 45 s on this NVMe disk) and a DS
ROM were imported into a separate empty library. All files matched the originals
byte for byte and both appeared in that library. They were not launched there
(no emulator installed in it), which is MP2.

Running a game still happens from the unpacked files. Emulator packages are
worth adding only for a locally modified emulator or an offline second PC; no
emulator has been modified so far.

## Implementation sequence

1. **MP0 — Manifest/receipt contract.** *(Done; schema 2 dropped the required receipt.)* Specify a versioned JSON schema and create
   a small synthetic fixture. Keep local instance IDs out of portable identity;
   define input/output artifact references and runtime requirements.
2. **MP1 — Reviewed local export/import.** *(Done from the CLI. Not yet: manager jobs, receipts, save conflict review, restart recovery: an interrupted import can leave a hidden `.marquee-import-*` folder to delete.)* Begin with a small already supported
   emulator game. Preview contents, space, dependencies and save conflicts. Use
   manager jobs, bounded extraction, no-replace publication, immutable receipts,
   idempotent registration and failure/restart recovery. Verify paths, hashes,
   duplicate members and resource limits before any library changes.
3. **MP2 — Cross-instance acceptance.** Export from isolated instance A, import
   into B with separate user data, launch/exit/relaunch and verify that A and B's
   saves/settings remain independent. Test incomplete/modified packages and
   unavailable runtimes. Only then add the larger PS3 direct-ISO path.
4. **MP3 — Optional private transfer.** Stream immutable payloads between paired
   instances with authentication, resumption and content verification. Reuse the
   registry and manager; network transfer does not imply installation or play.

A directory-based manifest plus payload is a suitable first internal form. An
archive extension/container can follow the local acceptance test; inventing a
custom compression or executable format is unnecessary. Self-contained exports
may temporarily require another full payload copy, so space checks and an
explicit destination are essential for a 38 GB disc.
