# Quiver integration roadmap

Created 2026-10-22; resumed 2026-10-25. **Q0 and Q1 implemented: pinned source/schema review and offline app-list discovery import. Q2a adds a pinned native-engine recipe pilot; durable manager jobs and game acceptance remain open.**

This track supports later native-port and catalogue expansion. It does not
change the direct-store implementation order in [ROADMAP.md](ROADMAP.md), add a
required launcher, or block Steam/Epic/GOG delivery. The owner explicitly scheduled this work on 2026-10-25.

## Intended outcome and architecture

Use Quiver's app-list conventions, release-provider patterns and selected code
to expand Marquee's managed ports without requiring another launcher. Marquee
continues to own its UI, library identities, runtime selection and sessions;
the [hidden manager](MARQUEE_MANAGER.md) owns durable preparation/install jobs,
verification and game-data setup. Quiver must not add a competing job authority.

Quiver uses C#/.NET and Avalonia; Marquee uses Rust and Tauri/Svelte. The preferred
route is a Rust catalogue importer and selective adaptation of useful logic and
tests. A .NET service, embedded Quiver UI, wholesale fork and replacement of
Marquee's core are outside this plan. Revisit that decision only with a concrete
benefit and an explicit lifecycle/distribution design.

An imported listing is discovery metadata, not an executable recipe or proof of
ownership, compatibility, authenticity or game-data readiness. An installed port
engine becomes playable only after its adapter validates the required inputs.

## Sequencing

| Phase | Scope | Depends on |
| --- | --- | --- |
| Q0 | Pin upstream, review reuse rights and choose pilot entries | **Done 2026-10-25** |
| Q1 | Offline app-list import, normalization and review | **Done 2026-10-25** (Q0) |
| Q2 | Reviewed native Linux installation pilot | Q1; shared persistent jobs and installation recovery |
| Q3 | Catalogue refresh, release selection and update review | Q2 |
| Q4 | Optional mod adapters | Q2/Q3; game-specific mod and save contracts |
| Q5 | Controller and kiosk interaction improvements | Separately scheduled; can proceed after Q0 independently of Q1–Q4 |

Complete and review one phase before expanding scope. Q1 can ship as discovery
only; do not delay it for a universal installer or the optional mods/kiosk work.

## Q0 — Reuse inventory and pilot selection

- [x] Pin the exact Quiver commit reviewed; record its version, source URLs and
  the files/algorithms/tests proposed for reuse. Recheck current code rather than
  treating the initial assessment as a quality or security audit.
- [x] Check root, core and file-level licenses, inherited notices and any copied
  dependencies. Preserve applicable copyright and MIT notices for copied or
  adapted code in a third-party attribution record and packaged notices.
- [x] Keep Marquee branding, identifiers, data directories and update endpoints.
  Quiver's name, logo and app icons are excluded by its branding policy.
- [x] Evaluate catalogue data, reviews, artwork and hosted API usage separately
  from the code license. Start with a user-provided local app list; no dependency
  on Quiver's hosted service or assumption of permission to mirror its database.
- [x] Inventory existing Marquee port/emulator capabilities and select two
  supported native Linux ports: one already represented and one genuinely new.
  Record game-data prerequisites and a known-good upstream release for each.

**Gate:** a pinned reuse inventory, retained notices, source/data policy and two
pilot fixtures. No upstream binary, script or catalogue entry is executed.

## Q1 — Reviewed app-list import

- [x] Implement bounded JSON parsing in core for the supported Quiver `apps`
  format. Reject malformed input as a failed import; never turn it into an empty
  library or overwrite personal data. Unknown fields get explicit diagnostics.
- [x] Map `name`, `project`, `repository`, `repositorySource`, `folderName`,
  `releaseAssetFilter` and `tags` into discovery records. Treat `appIconUrl` as
  optional remote artwork with separate provenance and validation. Preserve
  unsupported requirements in the review result rather than silently dropping
  fields that change installation or launch behavior.
- [x] Preserve original field values and source provenance alongside normalized
  values. Use source-scoped entry identity; repository alone is insufficient
  because several apps may share one release repository. Never merge by title
  or derive a writable installation path directly from `folderName`.
- [x] Keep an explicit mapping to existing Marquee recipes and installations.
  Re-import is idempotent and preserves titles, favorites, sessions and profiles.
  Manual apps and unsupported providers remain discovery/manual entries.
- [x] Expose preview/approve through shared core commands and events, desktop,
  browser and CLI. Show Add, Already present, Needs recipe and Unsupported, with
  actionable reasons. Preview cannot install or run anything.
- [x] Test malformed/oversized lists, unknown fields, path-like names, duplicate
  entries, multiple apps per repository, repeat import and preserved user edits.

**Gate:** import a local fixture, review and approve supported entries, restart,
and re-import with stable identities and no downloads or executable side effects.

## Q2 — Native installation pilot

- [ ] Adapt selected release-provider and asset-matching logic into Rust. Begin
  with GitHub; add GitLab only after verifying its API, pagination, authentication
  and asset-link behavior at the pinned implementation version.
- [ ] Resolve approved discovery entries to explicit Marquee recipes: OS/CPU,
  release/channel, selected asset, executable, argv/cwd, setup and save paths.
  Reuse current port/emulator installers where suitable instead of creating an
  independent Quiver installation tree. Ambiguous assets require review.
- [ ] Record digest provenance and enforce Marquee's integrity policy. Quiver's
  Verified label is attributed upstream information, not Marquee verification
  or a publisher signature. Missing or changed integrity evidence blocks normal
  automatic installation; catalogue metadata cannot weaken that policy.
- [ ] Use shared persistent jobs for progress, cancel, retry, restart recovery,
  safe staging/promotion and rollback. Keep settings, game inputs, saves and user
  mods outside replaceable program payloads; removal respects file ownership.
- [ ] Add readiness validation and authorized input/setup jobs for each pilot.
  Launch through the existing adapter/session service; downloading an engine or
  opening its setup window does not establish game readiness or playtime.
- [ ] Test wrong OS/CPU, ambiguous assets, digest mismatch, hostile archives,
  interrupted installs, rollback, repeat install and preserved user files.

**Gate:** both pilot ports install, complete setup, launch a real game, return to
Marquee and relaunch after restart. Capture the exact builds and lifecycle
evidence. Until then, report installation and gameplay acceptance separately.

## Q3 — Refresh and update review

- [ ] Add explicit source refresh with caching, rate-limit handling and useful
  offline/error states. Preserve approved entries when a source disappears.
- [ ] Show a reviewable diff for source edits, removed entries, repository moves,
  changed assets/executables, requirements and integrity evidence. Source refresh
  never deletes installations, overwrites user preferences or approves new code.
- [ ] Support release/version choice and pinning. Reconcile upstream release
  status with Marquee's policy; never promote unverified or blocked releases
  through automatic updates. Recheck digests for the selected immutable asset.
- [ ] Test renamed repositories, withdrawn releases, API failures, stale refresh
  responses and interrupted updates; preserve launchable offline installations.

**Gate:** refresh, review and update a pilot with rollback and preserved saves;
an unavailable catalogue cannot break an already installed game.

## Q4 — Optional mods

- [ ] Choose one pilot port and one provider, Thunderstore or GameBanana. Review
  provider terms, file licenses, compatibility and API behavior before reuse.
- [ ] Define supported game/engine versions, dependencies, conflicts, load order,
  file ownership and enable/disable/remove behavior. Resolve dependencies into a
  reviewed plan; never silently overwrite another mod's or the user's files.
- [ ] Reuse bounded downloads/extraction, persistent jobs and diagnostics. Keep
  mod management independent of catalogue import and core engine updates.

**Gate:** install, enable, disable, update and remove a compatible mod; test a
conflict and failed update; preserve the base game and saves across restart.

## Q5 — Optional controller and kiosk improvements

- [ ] Review Quiver's SDL input, focus, repeat, modal and kiosk behavior as design
  references. Port applicable behavior/tests into Marquee's existing navigation;
  do not introduce Avalonia or assume browser gamepad tests prove native support.
- [ ] Validate native Tauri and browser focus separately, including reconnect,
  scrolling, search, dialogs, error recovery and return from an actual game.
- [ ] If kiosk mode is scheduled, define an unlock/recovery path and distinguish
  restricted app controls from OS-level containment. Defer packaging/session
  restrictions until a supported deployment environment is selected.

**Gate:** a real-controller couch session can browse, launch, return and recover
from an error without a mouse; kiosk unlocking/recovery is tested if implemented.

## Resume checklist and implementation record

Start with Q0, then a small Q1 local-list fixture and review flow. Do not import a
whole community catalogue or enable unattended updates as the first increment.
Likely touchpoints are `catalog/ports/`, `crates/core/src/ports.rs`, the shared
library/job/API layers and the Ports/Library screens. Dynamic approved recipes
and their migrations require design: the current shipped port catalogue is
compiled in and is not already a user-extensible plugin system.

For every phase, record the upstream commit, reused files/notices, schema/API
changes, tests, real acceptance evidence and remaining limitations. Run the
repository's required checks before each implementation push. Leave unchecked
gates open until observed; estimates should follow Q0 rather than be invented
from the feature list.

## Reference sources

Initial assessment checked on 2026-10-22; upstream `main` URLs are mutable.

- [Quiver repository and app-list documentation](https://github.com/tgeorgiadis/quiver-launcher)
- [MIT license](https://github.com/tgeorgiadis/quiver-launcher/blob/main/LICENSE)
- [Branding policy](https://github.com/tgeorgiadis/quiver-launcher/blob/main/TRADEMARKS.md)
- [.NET/Avalonia project configuration](https://github.com/tgeorgiadis/quiver-launcher/blob/main/QuiverLauncher.App.csproj)
- [Platform asset matcher](https://github.com/tgeorgiadis/quiver-launcher/blob/main/lib/QuiverLauncher.Core/Services/PlatformAssetMatcher.cs)
- [App catalogue service](https://github.com/tgeorgiadis/quiver-launcher/blob/main/Services/AppCatalogService.cs)


## Q0 implementation record (2026-10-25)

Pinned upstream revision: `a27f9a7b1f8d188dc693aff9a6567a7d5101ab95`.
Reviewed the documented modern `apps` array, `Services/AppCatalogService.cs`,
`Models/GameInfo.cs`, its catalogue tests and the core platform matcher. Upstream
also accepts legacy arrays/sections; the first Marquee importer intentionally
supports only the explicit modern object/`apps` format and reports other shapes.
Upstream currently describes one personal app-list source rather than the older
subscription model. This does not authorize mirroring its hosted catalogue.

The root MIT license attributes SirDiabo and Thomas Georgiadis (2026); the core
MIT notice attributes SirDiabo (2025). Both notices and the reference inventory
are retained in `third_party/quiver/`. This is original Rust schema-compatible
code; no upstream runtime, dependencies, branding or artwork are copied.

The hand-authored local fixture is `catalog/app-lists/pilots.json`. Pilots:

| Entry | Marquee mapping | Upstream release checked | Game-data prerequisite |
| --- | --- | --- | --- |
| Ship of Harkinian | Existing `shipwright` recipe | Shipwright 9.3.1, `SoH-Dewey-Bravo.appimage` | Compatible Ocarina of Time game data and port-native setup |
| 2 Ship 2 Harkinian | New; needs a reviewed recipe | 2ship2harkinian 5.0.1, `2Ship-Battler-Bravo-Linux.zip` | Compatible Majora’s Mask data and port-native setup |

These are upstream Linux asset candidates, not observed gameplay acceptance. No
release asset or retail data was downloaded or executed during Q0. Q2 must pin
asset digests, validate setup/save contracts and observe launch before claiming
the new port works. Q0's source/data/fixture inventory is complete; its live-game
evidence belongs to Q2.


## Q1 implementation record (2026-10-25)

Tools → **App lists** accepts a user-selected local JSON file in the pinned modern
`apps` format. The source name is a logical namespace, not an install path; reuse
it for re-import. Preview shows Add, Already present or Update discovery details,
a managed recipe mapping or Needs recipe/Unsupported, normalized tags, retained
requirements and expandable original fields. Explicitly select and import the
records. A preview never installs or launches anything. New list records stay in
Tools discovery and do not add playable entries to Games.

Original Rust parsing supports the documented name/project/customDisplayName,
repository/provider, folderName, asset-filter, tags and optional icon fields.
Unknown entry fields (including launch commands, local paths and setup/mod
requirements) remain visible in the original record and block recipe reuse.
Unknown root fields are reported and retained in the source snapshot. GitLab,
manual and unsupported providers never silently become GitHub installers.
Remote icons are retained as unfetched metadata. Only a known shipped GitHub
recipe without external filters/unsupported fields receives a mapping; it is
not proof of installed game data or readiness. The current new pilot needs a recipe.

Bounds: 384 KiB UTF-8 JSON, 500 apps, bounded strings and tags, standard JSON
recursion limit. Invalid JSON/field types, oversized input and duplicate
source/provider/repository/folder identities fail the whole preview. Multiple
apps in one repository require distinct folderName values. Path-like folders
remain Unsupported and are never converted into writable paths. Legacy root
arrays/sections are not supported by this first importer.

New additive SQLite tables: `app_list_reviews`, `app_list_sources`,
`app_list_entries`. Sources retain original JSON, SHA-256 and import timestamp;
entries retain original values and normalized discovery metadata. IDs hash the
source/provider/repository/folder identity, not the display title. A review
expires after 30 minutes; a newer preview for the same source invalidates it.
Approval checks every selected ID and publishes transactionally. Re-import only
changes explicitly approved discovery records, and never removes absent entries,
installs software or touches game titles, favorites, sessions, profiles or saves.
This is separate from full source-refresh/removal/version-diff work in Q3.

Shared API: `preview_app_list {source,json}`, `approve_app_list {token,ids}` and
`app_list_entries`. CLI (uses the normal library lease; use an isolated data dir
for experiments or close the main library first):

```sh
marquee app-list preview catalog/app-lists/pilots.json --source my-app-list
marquee app-list approve <review-token> <entry-id> [<entry-id>...]
marquee app-list list
```

Acceptance: full repository checks and production build passed. Unit tests cover
bounds, malformed lists, duplicate/multi-app repositories, unknown executable
requirements, unsafe folder names, unsupported providers, expired/cross-review
IDs, restart/re-import identity and preserved game preferences. The real CLI
preview/approve/list/re-import cycle passed across process restarts. Browser file
selection and preview/approve/re-import passed at desktop and 390 px, with no
horizontal overflow or JS errors. Fixtures were hand-authored locally; no hosted
Quiver catalogue, remote icon, engine binary or retail data was fetched by import.

Q2 remains open: the new port needs a pinned native recipe, durable manager
installation/setup and real input/save/launch acceptance. The checked local N64
input directory did not contain a compatible Majora’s Mask ROM. Do not report
2 Ship 2 Harkinian as installed or playable from this discovery slice.


Main-app rollout: two selected hand-authored pilot records were previewed and
approved under the `marquee native-port pilots` source. Existing edition IDs,
titles, favorites, artwork and runner overrides were checked unchanged. The
updated native app exposes both records in Tools → App lists. No game binary
was installed or launched by this rollout.


## Q2a — Pinned 2 Ship engine recipe (2026-10-25)

A shipped `2ship2harkinian` recipe now names the exact 5.0.1 Linux ZIP and
`2ship.appimage` member. Approved app-list records can open **Review managed
recipe**; this opens the existing port settings dialog, without downloading or
executing anything. Installation is still a separate explicit action. External
asset filters, setup fields and commands cannot alter the shipped recipe.

The original HarbourMasters repository now redirects to the canonical
`2ship2harkinian/2ship2harkinian` project. The recipe uses that canonical owner;
the old path is an explicit compiled import alias, not a general redirect rule.
Original list fields and source-scoped IDs are preserved. Re-preview/re-import
the original pilot list to refresh its recipe mapping.

Reviewed source: `8a24047fbce8915993804e7819f4df4fa591551f` (tag 5.0.1).
Pinned asset: `2Ship-Battler-Bravo-Linux.zip`, 16,262,941 bytes, SHA-256
`6cdd4fe06f3f9b095e99c86fef70b9265cc921fb6faa7ba1e18e94e90ed0b345`.
Installed `2ship.appimage`: 16,553,152 bytes, SHA-256
`2b1e34c84e4ec011e7c50e131786ff83454f1a9cf12f041e5efcec01c4cc2964`.
These identify the upstream bytes; they are not publisher-signature verification
or gameplay evidence. The verified archive contains the AppImage and readme only.

The new pinned-recipe path selects the exact tag/asset and canonical GitHub URL,
refuses changed upstream digests, verifies the archive against the compiled pin,
and validates ELF64/x86-64 plus the extracted executable hash before promotion.
It never falls back to the legacy HTTPS-only/no-digest behavior. Repeat install
reuses only a matching executable; a damaged program is repaired from the pinned
release while the existing user-file preservation path retains settings. The
reviewed release remains pinned until its recipe is deliberately updated.

The isolated install and real corruption/repair test passed without executing
the engine or obtaining game data. Test settings survived unchanged. Full checks
and production build pass; browser discovery-to-recipe navigation shows **Install**,
the 5.0.1 pin and missing-game-data requirements at desktop and 390 px without
JS errors or horizontal overflow. Main-library game data is untouched.

**Q2 is not complete.** This slice uses the existing foreground port installer;
it has not moved native ports into durable manager-owned jobs. It does not test
AppImage startup/FUSE behavior, supported-ROM preparation, controls, saves,
gameplay, process/session ownership or relaunch. No compatible Majora’s Mask
input was found in the checked local collection. Tools/Game views retain
**setup unchecked** after installing an engine; they do not claim Ready to play.
Next: durable install/setup receipts and recovery, then a real supported-ROM
pilot with native save/relaunch acceptance. Do not copy another game's ROM or
mark the engine as a complete game installation to close that gate.

Primary references: [pinned source README](https://github.com/2ship2harkinian/2ship2harkinian/blob/8a24047fbce8915993804e7819f4df4fa591551f/README.md),
[release](https://github.com/2ship2harkinian/2ship2harkinian/releases/tag/5.0.1).

## Q2b — Detached engine installation (2026-10-25)

Native engines now install through the existing private `marquee-manager`
process. Start returns after a durable checkpoint; closing Marquee leaves the
accepted worker running. The port dialog restores persisted progress on reconnect,
allows cancellation and retries interrupted/failed jobs. CLI `ports install`
waits for that same worker; exiting the CLI does not cancel it.

Protocol v6 accepts only shipped recipe IDs. The manager resolves the release,
URL, executable and destination itself, and never opens `library.db`. Jobs and
immutable attempt receipts use additive tables in `manager/jobs.db`. Receipts
record the compiled recipe hash, release tag, relative executable and output
SHA-256/size. On reconnect the library owner rechecks the output and publishes
settings and receipt identity in one transaction. Failed/cancelled retries keep
the previous successful receipt. Existing pre-manager installs remain usable.

The existing bounded installer/extraction checks moved into the manager without
loosening checksum rules. A deterministic engine backup and filesystem syncs
recover the rename window; retry restores the old tree if publication did not
finish and removes only that recipe's abandoned staging directories. User game
files/settings are preserved. Download cancellation is checked between chunks;
extraction cancellation waits for its next phase boundary. Once publication
starts it finishes. A manager stop drains accepted work and remains stopped until
explicit restart. A crash marks running jobs **interrupted**; retry downloads
fresh bytes, rather than claiming HTTP partial-download resume.

Live isolated acceptance passed with pinned 2 Ship 5.0.1: short-lived requester
exit, completion without any library database, forced manager crash/restart,
cancellation, retry, preservation of settings and the prior receipt, and eventual
library publication after explicit manager restart. The engine was not executed.
Ignored evidence: `data/manager-pilot/quiver-q2b/detached-install-receipt.json`.

This completes engine-job durability, not the full Q2 setup/play gate. Game-data
inspection/setup, engine execution, native session ownership/playtime, saves and
relaunch still need their own acceptance. The owner has now supplied an Ocarina
of Time RAR; inspect its actual ROM revision before selecting a Shipwright recipe.
