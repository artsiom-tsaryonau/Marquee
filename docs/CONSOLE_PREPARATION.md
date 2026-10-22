# Console collection: inventory and preparation plan

Updated 2026-10-22. The owner reports adding **Switch, PS3, PS4, original Xbox and
Xbox 360 games**. The source tree has now been inventoried read-only using filesystem metadata and
ZIP headers. Inner formats not inspected remain unknown; no console game, firmware
or key material was installed or changed.

[Marquee manager](MARQUEE_MANAGER.md) owns preparation jobs; Marquee owns library
and launch. This document owns the platform backlog. [Emulators](EMULATORS.md)
records current tool behavior. Do not equate catalogue entries with game adapters.

## Current repository evidence

As of this pass, `catalog/systems/` has no `switch`, `ps3`, `ps4`, `xbox` or
`xbox360` descriptors. Registering those roots is therefore not supported by the
current descriptor-validated library flow. `metadata.rs` can retain provider
platform identifiers without providing import or launch support.

| Platform / proposed ID | Existing tool entry | Missing before library Play |
| --- | --- | --- |
| Nintendo Switch / `switch` | No managed emulator entry found | Select and validate a runner; content inventory, prerequisites, installation/launch adapter and live evidence |
| PlayStation 3 / `ps3` | RPCS3 managed install/open; firmware requirement recorded | Firmware installation recipe, content/installed-title inventory, package or folder preparation, launch/profile/save mapping |
| PlayStation 4 / `ps4` | shadPS4 managed install/open, experimental | Package or prepared-folder inspection, title/version relationships, explicit supported install method and per-game launch/validation |
| Original Xbox / `xbox` | xemu managed install/open; BIOS/MCPX/HDD requirements recorded | Firmware/HDD configuration, input format inspection, tested conversion only when needed, library/launch/save adapter |
| Xbox 360 / `xbox360` | No managed emulator entry found | Select and validate a runner on the target Linux host; content inventory, setup, launch and compatibility evidence |

This is local code evidence, not a claim about current upstream emulator support.
Runner selection, package handling and exact supported formats must be verified
against pinned upstream documentation/builds during implementation. Do not add
system descriptors with a generic `{rom}` command merely to populate the shelf.

## Observed source inventory

Root: local `sample_images/` in this checkout (ignored by git in full, including
key files). Observed 21 files, 120,861,997,153 bytes total across existing and new
platforms. Full paths/sizes and ZIP-header summaries are in ignored
`data/manager-pilot/source-inventory.json`; no key contents were read.

| Platform | Observed source shape | Preparation status |
| --- | --- | --- |
| PS3 | God of War III `.7z`, about 35.8 GB compressed | Inner layout not inspected; no PS3 adapter |
| PS4 | The Last of Us Remastered `.pkg.rar`, about 47.2 GB; separate Shadow of the Colossus `.torrent` metadata | Filename is only a format hint; RAR payload/torrent files not installed or fetched |
| Switch | Firmware ZIP containing `.nca`; Nightshade ZIP containing `.nsp`; Zelda base ZIP containing `.xci` plus `.url`; separate DLC ZIP containing `.nsp`; two `.keys` files | Containers recorded separately; no base/update/DLC identity or key validity inferred |
| Xbox | Splinter Cell `.iso.rar`, about 6.3 GB | Inner layout not inspected; requires xemu-specific format/configuration work |
| Xbox 360 | Vanquish `.iso.rar`, about 6.2 GB | Inner layout not inspected; runner/host/format work remains open |

The small Sonic Genesis ZIP was the M1 real preparation pilot in an isolated
database. Its source hash was unchanged, and no other collection files were
extracted. The existing user library remained at schema 5 with 856 editions and
29 sessions during inspection; it was not reset. The new build migrates libraries
additively to schema 6 on opening.

## P0 — Read-only inventory (in progress)

- [x] Establish supplied source root: checkout `sample_images/`; inspect only that
  tree and existing library configuration. Additional roots are not assumed.
- [ ] Complete inventory beyond supplied archives: torrent records, unpacked folders and existing emulator installs
  without changing them. Record path/volume, size, platform hint and attribution.
  Enumerate archive headers with existing safety limits; defer full hashing of
  large payloads until selected work requires it.
- [ ] Classify outer container separately from inner content: archive, disc image,
  game folder, installable package, firmware, update, DLC, key/license material or
  unknown. An extension such as `.iso` or `.pkg` is ambiguous across platforms.
- [ ] Record base title/version relationships only with reliable metadata. Preserve
  region/revision variants and all source files. Handle encrypted, split,
  password-protected or incomplete inputs as explicit blocked cases.
- [ ] Produce a reviewable inventory and choose one representative base game/input
  per platform for later acceptance. Do not install the entire collection as a batch.

Inventory output should include `source_ref`, outer/inner kind, platform evidence,
size, known title/version/content role, current preparation state, missing
prerequisites, proposed recipe and next action. Exclude secrets and key contents.
Retain uncertain items in a source/preparation view; do not fabricate playable
library installations to make them visible.

## P1 — Common requirements before platform recipes

Use manager M1/M2 contracts: stable artifact/receipt IDs, reviewed destination,
space estimate, restart recovery, user-action states and idempotent library
publication. Distinguish base games from updates/DLC and firmware at the model
level before importing any package-based collection. Never feed all `.iso` files
through the existing PS1/PS2 size heuristic; explicit platform evidence/review is
required. Never extract a known Xbox/Switch archive through the legacy intake
path just because its contents include a familiar extension.

An archive is a source container, not an installation recipe. Unknown or
unsupported formats remain retained sources with a useful reason and next action.
Existing emulator installations can be inventoried/adopted without taking over
their configuration, accounts or saves automatically.

## P2 — Per-platform recipe spikes

The order of spikes is chosen from the actual inventory and a usable runner on
this host, not console generation. Tool availability alone does not close a gate.

| Platform | Spike deliverable and acceptance |
| --- | --- |
| PS3 | Pin RPCS3 build; distinguish prepared disc folders, packages and installed titles using validated metadata; support one chosen input; install firmware through a tested method, preserve keys/licenses privately; select the real boot target and map saves; reject update-only inputs as base installs |
| PS4 | Pin shadPS4 build; inspect one actual input; verify the supported package/prepared-directory workflow and base/update requirements; installation creates a receipt only after outputs validate; a title that cannot launch stays explicitly blocked/untested |
| Original Xbox | Pin xemu build; validate BIOS/MCPX/HDD and write only managed configuration with receipts; determine whether the supplied image is accepted or needs an explicit reviewed conversion; retain original media and user HDD/saves |
| Switch | Select an available runner after checking target-host and distribution requirements; inspect actual content/title relationships; verify firmware/key configuration without logging secrets; implement one supported base-game path before updates/DLC; no assumed universal package support |
| Xbox 360 | Select a usable runner/build and target-host execution method; inspect actual disc/folder/package layout; implement one supported base-game path with clear host/runtime limits; define profile/save locations before updates or extra content |

For every spike, record exact build, input kind, identity evidence, dependencies,
storage behavior, install/repair procedure, executable/argv, settings/saves and
known limitations. Use upstream sources at implementation time; no runner choice
is approved merely by appearing in an older expansion list.

## P3 — Library and live acceptance per platform

- [ ] Add validated system descriptors, scanner/installed-title inventory and
  runner capabilities together; avoid hard-coded package handling in Svelte.
- [ ] Register each verified base installation once; attach updates/DLC to that
  installation and retain independent editions, sources and saves.
- [ ] Exercise interrupted preparation, offline volume, missing firmware/key,
  unsupported/encrypted input and failed launch with useful recovery actions.
- [ ] Observe a selected real game's boot, input, save/load, exit/return and restart
  on the target host. Record a title-specific result; do not infer all games work.
- [ ] Verify update/repair rollback and uninstall preserve selected save/HDD/source
  retention rules. An opened emulator window is not a game session.

These gates remain open for all five systems. Existing PS1/PS2 and other platform
acceptance requirements remain independent. Downloading more games does not
close any preparation or compatibility gate.
