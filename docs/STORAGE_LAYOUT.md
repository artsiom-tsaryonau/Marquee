# Collection storage and retention

Storage design and first migration tooling, 2026-10-25. The offline relocation
tool preserves existing library and receipt paths with compatibility links.
The current manager, library and emulator paths remain authoritative; a unified
artifact registry and complete per-category relocation remain future work.
The [Storage settings screen](STORAGE_MANAGER.md) now provides locations, usage,
a future-intake collection folder and an additional BIOS input folder.

Keep three independent lifecycles: original sources, playable installations,
and mutable user data. Firmware originals have their own versioned vault;
emulators use managed working copies. A title can have several source artifacts
and installations without duplicating its identity in Games.

## Recommended eventual layout

```text
Marquee/
  sources/
    games/<platform>/<title and verified ID>/
      base/
      updates/<version>/
      dlc/<content ID>/
      provenance/
    firmware/<platform>/<version or model>/
    licenses/<platform>/<verified title ID>/
    native-data/<project>/<input revision>/
  library/<platform>/<title and verified ID>/
  user-data/<emulator>/<profile>/
  programs/<emulator>/<build>/
  music/
  backups/
```

The readable library/programs/user-data split above is the desired product
layout. Marquee currently keeps
programs under `<data>/runners`, emulator profiles under `<data>/<emulator>`, and
prepared game payloads under `<data>/manager/installations/<receipt ID>`. Its
receipts refer to those paths. `--data-dir` selects an instance's whole data root;
it does not yet configure every storage class separately. Keep opaque receipt
paths intact. The first relocation tool deliberately preserves them through
parent-directory compatibility links instead of rewriting immutable receipts.
`storage.collection_root` configures the base for new-platform immediate archive
intake; existing platform roots take precedence and manager destinations do not
change. Its sources directory also appears in the storage inventory.
The UI can provide readable platform/title organization independently of them.

The source repository should hold code, recipes, tests and documentation.
The first migration relocates ignored `sample_images` and ROMs onto a dedicated
collection root, preserving their old paths as links. Pilot data still needs a
separate ownership-aware move. Do not remove collection files or compatibility
links as part of code cleanup.

## What belongs where

- Sources: original RAR/ZIP/7z, torrent metadata and its exact payload, original
  discs/cartridges, manifests and download provenance. Preserve base/update/DLC
  roles. A filename is a hint until format/title metadata verifies it.
- Library: verified extracted ISOs, game folders and installed package outputs.
  Only supported adapters become launchable. A validated ISO can be the runtime
  payload directly; avoid a redundant second copy when a receipt can reference
  an immutable artifact safely. Sharing/reference-counting is future work.
- Native data: compatible original inputs and generated O2R/assets, associated
  with the exact port recipe. A Shipwright input is not interchangeable with an
  arbitrary Ocarina build. Keep generated data outside replaceable engine files.
- Firmware vault: original PS3 PUP, console/model-specific BIOS and MCPX files,
  plus a clean xemu HDD template. Record version/model, size and SHA-256. Never
  overwrite one version with another just because both files have the same name.
- Working profiles: installed firmware, xemu's *mutable* HDD, memory cards,
  battery saves, emulator-version-specific states, settings and user mods.
  Updates replace the program, not these files. Keep states separate between
  engines; do not imply that changing an emulator migrates saves.
- Licenses: existing per-title license material stays distinct from firmware,
  associated with its title/content identity. This layout adds no license import
  or key-validation capability.
- Music/media: original audio/video sources and their playable copies retain
  separate provenance. Game links can be removed without deleting global albums.

## Registry and deletion policy

The proposed artifact registry should retain SHA-256, size, platform, verified
identity/role, source location, source/version provenance, retention status and
all installation dependencies. Installation records should retain recipe/build,
inputs, output hashes, path, readiness evidence and save/profile ownership.
Missing and deliberately deleted sources remain records rather than disappearing.
Manager receipts already cover part of this relationship; a unified registry,
relocation and dependency-aware cleanup are still planned.

Keep the source until preparation verifies its outputs and the game has passed
launch, native-save persistence, clean exit and relaunch. Afterward, redundant
archives can be eligible for removal if policy permits, no dependent job needs
them, and recovery remains possible. Do not delete unique original inputs,
firmware versions, licenses, native-port prerequisites or active torrent payloads
as if they were temporary extraction files.

A torrent seeds the exact files and paths described by its metadata. An installed
or transformed game normally cannot substitute for that payload. Keep active
seeds in an unchanged source location; stop the transfer before relocation or
removal. Torrents are transport metadata, not game installations or proof of
source authenticity.

Back up the library/manager registry together with saves, mutable HDDs, licenses,
settings and essential originals. Redownloadable program builds and reproducible
caches are lower priority. Use consistent database backups; copying a live SQLite
DB alone can miss its WAL. A working restore test matters more than another
unverified copy.

## Migration order

1. Inventory paths and hashes, classify roles, and identify active jobs, torrents
   and sessions. Preserve the current receipt/library relationships.
2. Add configurable storage roots and a receipt-aware move operation with free
   space checks, hash verification, interruption recovery and atomic path updates.
3. Move one verified source/installation cohort. Relaunch it and confirm its
   existing saves before expanding the move.
4. Offer cleanup based on retained-source policy and dependencies, with a
   reviewable deletion plan. Keep removed-source history.

## Offline relocation tool

`scripts/storage-migrate.py` requires Python 3 and Linux `renameat2`. It accepts
explicit directory mappings into a chosen collection root. It hashes regular
files, records links without following them, and writes a private inventory and
recovery journal to `<collection>/.storage/migration.json`. The journal records
the original directory identity and per-file SHA-256, size and mode. It is a
relocation inventory, not verified game metadata or a global dependency registry.

```bash
python3 scripts/storage-migrate.py plan --root "$HOME/Games/Marquee" \
  --mapping "$PWD/sample_images=sources/imported" \
  --mapping "$PWD/roms=library/imported-roms"
python3 scripts/storage-migrate.py apply "$HOME/Games/Marquee/.storage/migration.json" \
  --only-source "$PWD/roms"
# Validate a game before applying the remaining cohort.
python3 scripts/storage-migrate.py apply "$HOME/Games/Marquee/.storage/migration.json"
```

Use quiescent input trees: no downloads, extraction, scans or game sessions writing
inside them. The tool takes a migration lock, refuses changed inputs or occupied
destinations, and only supports moves on the same filesystem. No second payload
copy or extra payload-sized free space is required. After the atomic rename it
creates an old-path compatibility link and verifies the destination hashes.
There is a short rename/link interval: run this as offline maintenance.

Rerun `apply` with the same journal after an interruption. Recovery recognizes
the directory's device/inode even if the rename completed before the journal
update. `rollback JOURNAL` restores verified, unchanged trees and removes only
the exact compatibility links created for them. It refuses to overwrite files
or roll back changed contents. Keep the journal outside version control; it can
contain filenames for private firmware/license material. Do not remove the
compatibility links while any database, receipt, wrapper or saved manifest still
uses an old path.

Library root registration recognizes an existing root's resolved destination.
Selecting the relocated platform folder again returns the original root and
retains its old media paths, game IDs, favorites and history. A different platform
assignment still reports a conflict. Avoid registering overlapping parent/child
folders as separate roots.

This first pass targets the repository's `sample_images` and `roms` trees.
Mixed original inputs remain under `sources/imported/<existing platform>`;
their names alone are insufficient to split firmware, licenses and games safely.
Receipt-owned installations, emulator working profiles and active streaming or
torrent data require their own coordinated migration. Do not apply this tool
blindly to a whole application data root: canonical roots also determine instance
and streaming ownership.

## Completed local migration — 2026-10-25

The owner's collection now uses `~/Games/Marquee` for these two physical trees:

| Existing compatibility path | Physical destination | Regular files | Bytes |
| --- | --- | ---: | ---: |
| `<checkout>/sample_images` | `~/Games/Marquee/sources/imported` | 22 | 174,298,281,751 |
| `<checkout>/roms` | `~/Games/Marquee/library/imported-roms` | 1,686 | 11,591,239,678 |

Both cohorts passed SHA-256 comparison after relocation. The 858 registered media
files remain available; editions (including favorites and play history), roots,
media records, preparation receipts and settings match the pre-migration snapshot.
Sonic 2 launched from its preserved ROM path into BlastEm on the private display,
with a running stream session confirmed and then stopped. This automated smoke
test does not constitute a new gameplay or save-state acceptance test.

The private recovery journal is `~/Games/Marquee/.storage/migration.json`.
The source checkout retains ignored compatibility links, not second copies.
Nothing was deleted. Keep those links for existing references. Program binaries,
working firmware, saves, receipt-owned installations and torrent/streaming pilot
data retain their existing paths. Their next migration needs category settings
and coordinated owner shutdown/restart; portable package export is still separate.

See [manager ownership](MARQUEE_MANAGER.md),
[torrent storage](TORRENTS.md), [console preparation](CONSOLE_PREPARATION.md) and
[portable package proposal](PORTABLE_PACKAGES.md) for their separate boundaries.
