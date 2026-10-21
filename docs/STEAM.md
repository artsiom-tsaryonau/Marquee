# Direct Steam integration

## S1A increment 1: read-only installed inventory

Implemented 2026-10-21. `marquee steam-inspect` and the shared JSON command
`inspect_steam` report native/Flatpak clients, registered libraries, local app
manifests, per-library scan completeness and warnings. This is discovery, not a
connected account or a completed S1A installation/launch flow.

Native roots include XDG Steam storage, `.local/share/Steam`, `.steam/steam`
and `.steam/root`; existing aliases are canonicalized. Flatpak uses its isolated
Steam data root and checks the known user/system package metadata before calling
the client available. No install, login, online sync or Steam command is run.

All numeric `libraryfolders.vdf` entries are considered, supporting modern
objects and old path values. Registered unavailable roots remain explicit,
with incomplete scan status rather than being reported as empty libraries.
Repeated physical library paths are scanned once. A shared native/Flatpak
library retains both client choices; distinct file copies keep separate stable
installation IDs. IDs use library location plus app ID, never title matching.

The original bounded KeyValues reader handles objects, quoted/unquoted tokens,
UTF-8/BOM, escaped quotes/backslashes and line comments. It refuses includes,
conditional directives, NULs, unbalanced input, excessive nesting, oversized
files/tokens and duplicate identity fields. This is a reviewed subset for Steam
text files, not a Source-engine script interpreter. Format reference:
[Valve's KeyValues implementation](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/tier1/KeyValues.cpp).
No upstream implementation code is bundled.

Each manifest must agree with its filename's numeric app ID. A safe single
installation-folder name is required. Names, build ID, size, state flags, file
presence and client associations are retained. Only state flags exactly `4`
are conservatively marked manifest-complete; pending/unknown states remain
separate. An install directory or complete manifest does not prove ownership,
application type, game readiness, compatibility or successful launch. Steam
runtime/tool manifests are not silently classified as games.

Malformed manifests produce per-file warnings without hiding other entries.
Missing/malformed library configuration warns and scans only the default root;
it never claims every library was found. Limits bound file size, directory
entries and manifest/library counts. No source file is modified, and no
credentials/login cache are read.

## Verification

Eight unit/fixture tests cover parsing, bounds/directives, modern/legacy roots,
shared and aliased paths, distinct copies, malformed/partial/missing media,
offline roots and identity/path validation. Tests use temporary directories.

A read-only host inspection found one native client, one library and four
manifests, with zero parsing warnings. One manifest had both the conservative
complete flag and an existing install directory. A repeat produced identical
installation IDs; SHA-256 comparisons confirmed all five source files (library
configuration plus manifests) were unchanged. Both commands used isolated
Marquee data directories. Flatpak, game launch and controller/return behavior
were not exercised live.

## Next S1A increments

- Persist Steam products/installations and personal overrides transactionally,
  preserving disc/port data and identities. Reconcile unavailable roots and
  incomplete scans without deleting cached installations. Future schemas must
  follow v5 and be backed up before testing against a real collection.
- Project into Library/game pages with explicit install/client states and
  artwork fallback. Keep provider/source and runtime separate. Account-owned
  sync is not implied by installed manifests.
- Hand off to the selected official Steam client; reconcile install changes.
  Do not credit client/helper process lifetime as gameplay.
- Observe a real user-owned game launch and return, restart preservation,
  native/Flatpak selection and unavailable-client behavior before closing S1A.

S1B owned-library sync/installation handoff, Epic, GOG and managed compatibility
remain separate roadmap milestones. No Steam password is collected.
