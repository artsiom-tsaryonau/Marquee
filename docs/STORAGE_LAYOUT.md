# Collection storage and retention

Proposed 2026-10-25 from the owner's existing collection. This is a migration
proposal, not a completed move or an implemented global artifact registry.
The current manager, library and emulator paths remain authoritative.

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
layout, not a set of folders to rename into today. Marquee currently keeps
programs under `<data>/runners`, emulator profiles under `<data>/<emulator>`, and
prepared game payloads under `<data>/manager/installations/<receipt ID>`. Its
receipts refer to those paths. `--data-dir` selects an instance's whole data root;
it does not yet configure every storage class separately. Keep opaque receipt
paths intact until a migration operation can update all references atomically.
The UI can provide readable platform/title organization independently of them.

The source repository should hold code, recipes, tests and documentation. The
current ignored `sample_images`, ROMs and pilot data are temporary collection
locations. Eventually migrate production content onto a dedicated collection
root, preserving the original sources and updating tracked paths through Marquee.
Do not remove these files as part of code cleanup.

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

No collection files were moved by this proposal. See [manager ownership](MARQUEE_MANAGER.md),
[torrent storage](TORRENTS.md), [console preparation](CONSOLE_PREPARATION.md) and
[portable package proposal](PORTABLE_PACKAGES.md) for their separate boundaries.
