# Quiver integration roadmap

Created 2026-10-22. **Deferred; planning only. No Quiver integration is implemented.**

This track supports later native-port and catalogue expansion. It does not
change the direct-store implementation order in [ROADMAP.md](ROADMAP.md), add a
required launcher, or block Steam/Epic/GOG delivery. Resume when the owner
explicitly schedules this work.

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
| Q0 | Pin upstream, review reuse rights and choose pilot entries | Owner schedules this deferred track |
| Q1 | Offline app-list import, normalization and review | Q0 |
| Q2 | Reviewed native Linux installation pilot | Q1; shared persistent jobs and installation recovery |
| Q3 | Catalogue refresh, release selection and update review | Q2 |
| Q4 | Optional mod adapters | Q2/Q3; game-specific mod and save contracts |
| Q5 | Controller and kiosk interaction improvements | Separately scheduled; can proceed after Q0 independently of Q1–Q4 |

Complete and review one phase before expanding scope. Q1 can ship as discovery
only; do not delay it for a universal installer or the optional mods/kiosk work.

## Q0 — Reuse inventory and pilot selection

- [ ] Pin the exact Quiver commit reviewed; record its version, source URLs and
  the files/algorithms/tests proposed for reuse. Recheck current code rather than
  treating the initial assessment as a quality or security audit.
- [ ] Check root, core and file-level licenses, inherited notices and any copied
  dependencies. Preserve applicable copyright and MIT notices for copied or
  adapted code in a third-party attribution record and packaged notices.
- [ ] Keep Marquee branding, identifiers, data directories and update endpoints.
  Quiver's name, logo and app icons are excluded by its branding policy.
- [ ] Evaluate catalogue data, reviews, artwork and hosted API usage separately
  from the code license. Start with a user-provided local app list; no dependency
  on Quiver's hosted service or assumption of permission to mirror its database.
- [ ] Inventory existing Marquee port/emulator capabilities and select two
  supported native Linux ports: one already represented and one genuinely new.
  Record game-data prerequisites and a known-good upstream release for each.

**Gate:** a pinned reuse inventory, retained notices, source/data policy and two
pilot fixtures. No upstream binary, script or catalogue entry is executed.

## Q1 — Reviewed app-list import

- [ ] Implement bounded JSON parsing in core for the supported Quiver `apps`
  format. Reject malformed input as a failed import; never turn it into an empty
  library or overwrite personal data. Unknown fields get explicit diagnostics.
- [ ] Map `name`, `project`, `repository`, `repositorySource`, `folderName`,
  `releaseAssetFilter` and `tags` into discovery records. Treat `appIconUrl` as
  optional remote artwork with separate provenance and validation. Preserve
  unsupported requirements in the review result rather than silently dropping
  fields that change installation or launch behavior.
- [ ] Preserve original field values and source provenance alongside normalized
  values. Use source-scoped entry identity; repository alone is insufficient
  because several apps may share one release repository. Never merge by title
  or derive a writable installation path directly from `folderName`.
- [ ] Keep an explicit mapping to existing Marquee recipes and installations.
  Re-import is idempotent and preserves titles, favorites, sessions and profiles.
  Manual apps and unsupported providers remain discovery/manual entries.
- [ ] Expose preview/approve through shared core commands and events, desktop,
  browser and CLI. Show Add, Already present, Needs recipe and Unsupported, with
  actionable reasons. Preview cannot install or run anything.
- [ ] Test malformed/oversized lists, unknown fields, path-like names, duplicate
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
