# Marquee manager: architecture and implementation plan

Decision: 2026-10-22. **M1/M2 implemented for local archives: durable preparation runs in a separate `marquee-manager` process.**
Marquee is the launcher and library. `marquee-manager` is a hidden, user-local
background worker for custom acquisition and preparation, including FMHY-source
connectors, browser downloads and torrents. It ships with Marquee; a second
application, separate repository, or required management UI is not the first step.

This document owns the manager boundary, lifecycle, data contracts, migration and
acceptance plan. [Roadmap](ROADMAP.md) owns priority; [console preparation](CONSOLE_PREPARATION.md)
owns platform-specific work; [store integration](STORE_INTEGRATION.md) owns store
adapter details. This decision supersedes plans to put every acquisition worker
and authoritative job queue directly inside the launcher process.

## Product boundary

Marquee presents Library, Get/Install/Play and a concise Downloads view. Detailed
preparation, diagnostics and source cleanup are secondary management actions.
A manager failure must not prevent launching an existing ready installation.
Closing Marquee leaves accepted jobs running; stopping the manager is explicit.
No downloads start merely because the launcher starts or a catalogue is imported.

| Component | Owns |
| --- | --- |
| Marquee library/core | Game and edition identity, metadata, user preferences, installation projection, launch profiles and sessions |
| Marquee UI / CLI | Reviewed requests, truthful progress, user interaction, launch and repair actions |
| `marquee-manager` | Durable custom acquisition jobs, transport workers, source artifacts, storage/staging, preparation recipes, install receipts, recovery and cleanup |
| Store adapters | Store authentication, ownership, inventory and supported backend operations; manager can coordinate their long-running jobs |
| Official Steam client | Steam login, installation, updates, execution and Steam compatibility settings; Marquee observes/reconciles handoffs |
| Legendary / GOG adapter + gogdl | Supported Epic/GOG operations, driven through store adapters; no generic torrent replacement |
| Runtime/emulator adapters | Supported prerequisites, isolated configuration, install/validation recipes and launch construction; manager runs setup jobs, launcher runs games |
| `fmhy-marquee-scrapers` | Provider metadata/selection parsing and browser-worker implementations; no authoritative installation or global job registry |
| Browser companion/native host | Reviewed browser interaction, browser event attribution and a paired bridge to the manager; authentication stays in the browser |

FMHY is a source index, not a transport. Provider connectors produce selections;
HTTP, browser and torrent workers move bytes. A provider cannot declare a game
installed or supply an arbitrary shell command. Keep the current embedded torrent
engine behind its transport adapter; an external torrent client is not required.
An external-client adapter can be evaluated later against the same contract.

## Acquisition user flow

1. Select an understood source, store product or existing local package in Marquee.
2. Review acquisition/preparation, destination and prerequisites.
3. The manager acquires and prepares through supported recipes; browser/store
   interaction stays visible when required.
4. A validated receipt reaches the library, which reconciles readiness and offers
   its normal Play action. Folder import and standalone tools remain fallbacks.

Source metadata is not ownership. Supported acquisition uses configured authorized
sources, stores and existing files; Marquee does not bundle proprietary BIOS/ROMs.
[Roadmap status](ROADMAP.md#current-implementation-versus-target) distinguishes
current features from manager targets; [connector priorities](MEDIA_CONNECTOR_PRIORITIES.md)
own provider scope and [console preparation](CONSOLE_PREPARATION.md) owns the
five added platforms. Do not maintain a separate acquisition overview/specification.

## Existing code to preserve

| Surface | Current implementation / gap |
| --- | --- |
| `crates/torrent` | `librqbit` transport; reuse it rather than rebuild BitTorrent |
| `crates/manager/src/torrent.rs` / core transfer adapter | Manager owns torrent engines/history/settings; Core chooses library shares and publishes completed private-device roots |
| `crates/core/src/intake.rs`, `rar.rs` | Reviewed archive intake, bounded extraction, staging, intact arcade ZIPs and CUE normalization; no persistent universal preparation queue |
| `crates/core/src/instance.rs`, `paths.rs` | One instance lock and library path ownership; process split must respect them |
| `api.rs`, `hub.rs`, `server.rs`, CLI and Tauri | Shared commands/events and authenticated browser surface; evolve these through a manager client |
| `db.rs`, `library.rs` | Library identities and scanning; retain as sole library writer |
| `emulators.rs`, `ports.rs`, `source_builds.rs`, `purple.rs` | Specialized setup/install implementations; preserve receipts, rollback, isolation, cancellation and data protection before moving workers |
| Scrapers' `browser-companion/native` | Local worker JSON and paired protocol v1 prototype; no common acquisition authority or automatic library publication |

The implemented dependency direction is:

```text
Marquee core/CLI/Tauri → marquee-manager client/protocol → private Unix socket
                                            ↓
                                marquee-manager service/binary
                                            ↓
                              marquee-media (archive/catalogue/CUE/ISO)
```

`crates/manager` has no `marquee-core` dependency. `crates/media` contains shared
bounded archive inspection/extraction and disc helpers; core keeps its existing
public module paths through re-exports. Legacy immediate intake reuses these
helpers. `crates/core/src/preparation.rs` now contains only client coordination
and library receipt publication. Torrent workers now run in the manager; emulator and runtime workers retain
their existing hosting until adoption. Browser jobs support an opt-in manager
bridge; legacy JSON remains until explicit cutover.

## Implemented worker lifecycle (M2)

- One manager per canonical data directory, protected by
  `<data dir>/manager/instance.lock`. On-demand spawn detaches the worker from the
  launcher's session; closing the launcher does not stop accepted preparation.
- A short Unix socket lives below the mode-700 per-user temporary directory
  `marquee-manager-<uid>/<data-directory-hash>/ipc.sock` (mode 600). Both peers
  check Linux `SO_PEERCRED`. Long isolated data paths do not exceed socket limits.
  No manager TCP port or direct browser endpoint is opened.
- Protocol v4 uses bounded length-prefixed JSON, typed operations, matching request
  IDs and a version check before mutation. Maximum frame is 32 MiB. The worker
  advertises snapshot polling; event replay and active preparation cancellation are
  unsupported. Browser cancellation is a request/acknowledgement flow. The existing browser/native-host protocol is still a separate bridge.
- Job state/receipts use private `manager/jobs.db` schema 4; the worker never opens
  `library.db` or holds the library's instance lease. The launcher transactionally
  imports receipts into library schema 6 and sends a fingerprint-bound acknowledgement.
  Failed/lost acknowledgements remain replayable. The active frontend reconciles
  receipts even when its archive panel is closed, including after reconnect.
- **Stop manager** checkpoints/stops torrents and drains accepted archive
  preparation, then exits. It records `manager/stopped`; polling respects that
  stop. Explicit Start, Inspect, Prepare or an active torrent command
  re-enables the worker. Unexpected termination leaves an interrupted checkpoint;
  the next start reconciles state without automatically rerunning the job.
- Worker unavailability or a missing companion does not gate existing library
  loading/launching. `manager/worker.log` records startup errors. Registered outputs
  are immutable in this slice: no worker update/uninstall API exists, and repeated
  Start cannot replace files used by a running game. General cross-process mutation
  leases remain required before moving mutable runtime/update work into the manager.

Controls, without opening or locking `library.db`:

```sh
marquee manager status
marquee manager start
marquee manager stop                  # stop torrents; drain archive preparation
marquee prepare inspect /absolute/path/Game.zip --system genesis
marquee prepare start JOB_ID --review-token TOKEN_FROM_INSPECTION  # accept and detach
```

`prepare run` waits and registers through Core; `prepare list` reconciles pending
receipts through Core. These library-writing commands still need an available
library lease. UI controls use the shared `manager_status`, `start_manager` and
`stop_manager` API. Sources are retained throughout.

### Delivery and M1 cutover

The supported local production layout places `marquee-manager` beside
`marquee-desktop`/`marquee`. Normal `scripts/run.sh` desktop/build/web/dev entrypoints
build the companion; Tauri before-build/dev commands build it too. Missing or
changed companion inputs invalidate the cached desktop pair. Raw CLI development
needs `cargo build -p marquee-manager -p marquee-cli` first. An advanced absolute
`MARQUEE_MANAGER_BIN` override is available. A signed bundle, RPM/AppImage installer
and user-service registration remain release/distribution work; none is claimed here.

Before adopting M1 job schema 1, the client takes the manager lease and ensures
exclusive legacy-library ownership (or uses the new Core's already-held library
lease). It creates a mode-600 `jobs-m1-backup-<uuid>.db` using SQLite `VACUUM INTO`,
then changes job schema to 2 without rewriting payloads/paths/IDs. Old M1 writers
refuse schema 2. An older live launcher blocks cutover; no second engine starts
on its state. The daemon itself performs no legacy-library locking or migration.

The real earlier M1 Sonic pilot migrated with the same job/edition IDs and one
attempt, and a private schema-1 backup. Synthetic tests cover lock refusal,
abandoned checkpoints and schema preservation. For rollback, stop the worker and
all library owners first; retain a copy of the schema-2 state before considering
restoration of its M1 backup. A backup predates later writes; do not restore it
under a live worker or discard new receipts/sources.

### M2 verification

The complete repository checks and real `scripts/run.sh build` production pair
build passed. Checks cover process single ownership/concurrent starts,
isolated libraries, private socket modes, protocol/version/oversized-frame refusal,
abrupt death/stale socket recovery, graceful draining, pending receipts without a
library database, missing helper behavior, migration and library-side idempotency.

A built-browser check with the owner's Sonic archive accepted a job, closed the
launcher before publication, observed the same independent worker PID and pending
receipt (with no `library.db` file descriptor), then reopened Marquee. Registration
happened once without opening the import panel. UI stop/start passed; original
source SHA-256 and attempt count were unchanged. This is local Linux process and
browser evidence, not retail game boot/input/save acceptance or installer-bundle QA.

## Implemented browser job bridge (M4 transport slice)

The sibling scraper companion now has an opt-in manager authority. Its paired
native host invokes `marquee browser request --client-id EXTENSION_ID` with bounded,
typed stdin, retaining pairing credentials in the host. The CLI reaches the
private manager socket without taking the library lease. No new browser endpoint,
cookie export or shell-command interface is introduced.

- `browser_jobs` storage in schema 4 owns new reviewed selections, stable
  start UUIDs, Chrome download IDs, attribution, last-reported bytes, cancellation
  requests and local completion manifests. Existing preparation records are
  unchanged. Stable start replay and duplicate completion are idempotent.
- Selection/provider/URL and file-root checks run in the manager. Complete files
  are opened below the approved root without symlink traversal and hashed outside
  write transactions. A local hash is evidence, not verified release identity.
- The companion reconciles its cache from manager snapshots. Lost start
  acknowledgements never cause another automatic site click. Cancellation from
  Marquee remains pending until the browser applies/acknowledges it; open/refresh
  the companion. Progress is last reported, not an unattended polling promise.
- Library → **Downloads and archives** shows these jobs. Completed ZIP/7z/RAR
  files enter a linked explicit-platform review: the archive is rechecked against
  download-time SHA-256, and review plus link commit atomically. Repeated inspection
  reuses one job; changing its platform does not silently replace that review.
  Preparation still requires approval. Updates/DLC and unsupported packages stay blocked.
- The installer's explicit `--switch-to-manager` refuses active/corrupt legacy
  jobs, retains pairing and job records, backs up configuration/history and retires
  the JSON v1 write schema under its lock. A marker also blocks stale configured writers. Completed legacy jobs are not adopted automatically;
  no sources move. The live user registration is unchanged by tests.
- Chrome owns transfer. Stopping the manager leaves Chrome running; restart and
  companion refresh reconcile it. There is no browser pause/resume capability or
  automatic restart of an interrupted download. Old running manager binaries must
  be stopped/restarted with compatible builds before using the added operations.

Checks exercise native framing → CLI → independent manager with synthetic ZIP
bytes, attribution rejection, stable replay, restart, completion/history and
cancellation. A disposable current-UI browser check verifies history, pending
cancellation and acknowledgement. The synthetic browser flow now reaches a linked
review, preparation and registered library receipt; replay/restart and source
removal retain one job/receipt/edition. Injected link/publication failures roll back
and retry safely. Live provider-to-install/launch, historical adoption, reference
hash matching and packaged delivery remain M4 gates.

### Browser artifacts, receipts and version cutover

`browser_preparations` links each completed browser job to one preparation job.
The review and receipt carry immutable `browser_source`: provider selection,
browser job/download IDs, attribution and the original file path/size/SHA-256.
The library stores this provenance in its existing transactional receipt payload;
no library schema change is needed. Legacy local-job IDs and receipt encoding
remain unchanged when provenance is absent.

Inspect through the UI or `marquee browser inspect BROWSER_JOB_ID --system genesis`.
This records review only. The normal approval, preparation, receipt publication
and acknowledgement follow. Source deletion after preparation does not erase
provenance or trigger download/extraction again; registration can retry from the
validated outputs. Snapshots report missing/unavailable/size-changed sources
separately; that availability check is not a fresh content hash.

Manager schema **4** retires writers that could drop torrent receipt provenance.
A schema-2/3 upgrade first writes a mode-600 `jobs-v<version>-backup-<uuid>.db`, then
advances without rewriting existing payloads. M1 cutover still takes its
exclusive ownership backup before the subsequent upgrade. Private IPC **v4**
rejects v1/v2/v3 job/receipt clients before mutation. The browser/native protocol stays
v1 and pairing is unchanged. Use `marquee manager stop` to drain an older worker
before starting the compatible pair; explicit Stop alone supports v1/v2/v3 shutdown; requested Start can drain an older
worker and start its compatible replacement.
No job or receipt operation silently downgrades. Do not restore a backup under
an active worker; a pre-upgrade backup does not contain later jobs/receipts.

### Existing-source live play gate

The owner's existing Sonic 2 Genesis ZIP passed real local preparation, library
registration and a core/CLI launch using the managed BlastEm image. The owner
confirmed video, keyboard input, audio, quick-state restore and clean close;
Marquee recorded exit 0 and one 363-second play in the isolated library. See
[exact scope and remaining gates](EMULATORS.md#sonic-2-live-acceptance-2026-10-22).
This validates one preparation/launch recipe, not new browser-job acquisition,
physical gamepads, other platforms or universal compatibility. The next manager
milestone has a supported torrent archive-to-preparation/receipt path, plus
a real receive-only PS4 torrent transport pass; platform package installation
and actual new-acquisition game launch remain separate acceptance gates.

## Data and ownership contract

Keep transport, preparation and library readiness separate. Proposed records:

| Record | Required meaning |
| --- | --- |
| Acquisition job | Stable UUID, request/idempotency key, reviewed selection/destination policy, adapter capabilities, phase, attempts, checkpoint, timestamps and actionable error |
| Transport record | Backend identity (torrent info-hash or bound browser download ID), original source reference, transfer status and supported resume/progress semantics |
| Source artifact | Immutable identity plus file manifest, actual paths/volume identity, sizes/hashes and evidence, provenance, retention and seeding dependencies |
| Preparation plan | Versioned trusted recipe, explicit platform/content kind, required inputs, storage estimate, destination, dependency graph and review fingerprint |
| Installation receipt | Stable install ID, input artifact IDs, platform/title/version where established, base/update/DLC relationships, output manifest, recipe/runtime version and validation evidence |
| Library publication | Durable receipt ID and idempotency key; the library validates and maps it to its own edition/installation/profile IDs |

One artifact can produce multiple installations; an installation can consume
multiple artifacts. Store products and entitlements remain distinct identities.
Record unknowns rather than inferring identity from a title, extension or hash.
A locally computed hash proves neither release identity nor compatibility. Track
transfer integrity, reference-hash match, structure validity, prerequisite health
and observed launch evidence independently.

The manager owns a separate private job/receipt database and its own instance
lock. It must never take the launcher's library lock or directly mutate
`library.db`. The library remains the single writer of library identities and
sessions. Receipt delivery uses a durable outbox: persist receipt and publication
event together; the library imports idempotently and acknowledges only after
commit. If Marquee is closed, publication waits. On reconnect/restart, replay
unacknowledged receipts and reconcile; do not claim exactly-once network delivery.
The manager receipt is authoritative for its managed outputs; the library's
installation entry is a projection plus independent availability/launch checks.

Current storage is `<data dir>/manager/jobs.db` (private job/receipt state)
and `manager/installations/<job-id>` with sibling per-job staging. Each receipt is
the durable outbox item until transactional library acknowledgement. Broader
transport payload policies and configurable destinations remain planned. Existing torrent directories and archives stay in place through adoption.
Source artifacts, staging, installations, firmware, settings and saves have
separate retention rules. Uninstalling a game does not silently delete its source
archive or saves; cleaning an archive does not uninstall its prepared game.

## Job lifecycle and preparation

Acquisition: `queued → acquiring → downloaded`; a local-file request starts with
artifact inspection. Preparation: `inspecting → awaiting_review → preparing →
validating → prepared → awaiting_library → registered`. Registration means the
library accepted the receipt, not that a retail game passed a live boot test.
Review can be reused only while its fingerprint and destination policy are valid.
Either flow can enter `needs_user_action`, `interrupted`, `failed` or `cancelled`.
`paused` exists only for a backend that actually supports it. Separate transfer
and preparation records allow seeding to continue while installation finishes.

- Download completion records source bytes, then offers the appropriate plan.
  Do not scan arbitrary package downloads as games.
- ROM/disc plans use reviewed intake and system adapters. Preserve required CUE
  tracks and arcade sets; do not flatten directories or repair ambiguous names.
- Windows installers require a specific runtime/prefix recipe, an identified
  executable and validation. Unsupported installers remain `needs_user_action`.
- Console packages require platform-specific recipes. Base games, updates, DLC,
  firmware and keys are different inputs; see [console preparation](CONSOLE_PREPARATION.md).
- Preserve extraction limits/path checks and stage output before promotion.
  Estimate simultaneous archive, staging and final space, including cross-volume
  copy. Disk-full/cancel leaves the old installation and source intact.
- Serialize mutation by destination/runtime/prefix; coordinate with launch leases
  so an update/removal cannot replace files used by a running game. Keep scan/DB
  locks short. Cancellation of an interactive installer reconciles its real state.
- Crash recovery inspects checkpoints and output manifests. Retry publication or
  scan without downloading/extracting again. Never turn an ambiguous partial install
  into success, or silently redownload a missing retained source.

Readiness is a library/profile result: files available, recipe supported, content
validated to the available evidence, runner and prerequisites present, profile
constructed. Compatibility remains unknown until evidence establishes it. Expose
missing source, offline volume, missing firmware and unsupported package separately.

## Local process and API contract

The implemented modules are `crates/manager/src/protocol.rs`, `client.rs`,
`daemon.rs` and `service.rs`, with shared media helpers in `crates/media`.
Future transports must extend this boundary without a manager → core dependency.

Use a user-private Unix-domain socket on Linux with peer/user checks and a
versioned handshake. The browser continues through Marquee's authenticated HTTP
surface; do not expose the manager directly to websites. Pairing credentials and
store secrets remain backend-side. No new unauthenticated loopback endpoint.

The following broader transport contract remains planned beyond the implemented
M2 snapshot/operation protocol. Do not imply event replay or additional transports
are available merely because they appear here:

- Requests: capabilities, inspect/review, enqueue/adopt, list/get, cancel,
  supported pause/resume/retry, approve changed plan, receipts/acknowledgement and
  explicit cleanup. Exact command names are chosen at implementation.
- Envelope: protocol version, request ID, idempotency key, job/attempt ID,
  monotonically sequenced persisted events, typed error and capability flags.
- Reconnect: snapshot plus cursor replay; cursor expiry requires a fresh snapshot.
  Duplicate events/completions are harmless. Reject stale review fingerprints and
  unsupported protocol versions before mutation; use compatible packaged versions.
- Worker boundaries: provider/media identity, transport attribution, allowed roots,
  immutable completion manifest and diagnostics with secrets redacted. Existing
  browser/native protocol v1 needs an explicit bridge/version migration, not silent
  reinterpretation.

The manager is one process per data directory, on demand, as the logged-in user;
no root service or mandatory remote server. Resolve duplicate spawn races with its
own lock/socket handshake. Launcher shutdown detaches; graceful manager stop checkpoints/stops torrents
and drains accepted archive preparation.
An optional user service can follow packaging acceptance. Autostart/network resume
policy is explicit and preserves torrent opt-in settings. Manager unavailability
shows jobs offline with retry; existing Play does not depend on its socket. Launch
coordination must retain a lock/lease that the manager checks before mutation.

## Implementation sequence and gates

M1/M2 are implemented for local archives; M0 now has concrete DTO/storage/protocol
and migration coverage. The M4 browser transport slice above is implemented; broader transport contracts
and M3–M6 completion gates remain open. Each increment must leave
existing Play/import usable.

| Increment | Deliverable | Acceptance before continuing |
| --- | --- | --- |
| M0 — Contracts and inventory | Record existing state paths/formats; finalize DTOs, storage ownership, capabilities, review/receipt schemas and migration fixtures | Dependency direction is acyclic; library lock/receipt ownership and recovery cases reviewed; no source files moved |
| M1 — Local preparation slice (implemented) | Explicit-platform `ArchivePlan`, separate durable jobs/reviews/receipts, UI/CLI/API and transactional library consumer | ZIP/7z/RAR fixtures and real Genesis browser registration passed; restart, failed publication, duplicate acknowledgement, changed inputs and low-space checks covered; game launch gates remain separate |
| M2 — Hidden worker (implemented for local archives) | Separate binary, private socket/client, worker lease, companion build, stop/start and M1 ownership cutover | Process tests and real browser close/reconnect passed; registered outputs remain immutable; distribution bundles and general mutable-update leases remain later work |
| M3 — Torrent adoption (supported archive slice implemented) | Manager hosts adapters/state, single-owner cutover and linked archive reviews/receipts; unsupported package recipes and broader live/release gates remain open | Pause/restart/resume and retained history survive migration; no duplicate engine, lost discovery policy, forced redownload or scan of unsupported packages; seeding protects sources |
| M4 — Browser/provider bridge | Connect paired native host and provider workers to manager job authority; adopt historical completed files only after validation | Browser and host restart, manual attribution, cancel and duplicate events reconcile; one supported provider download reaches a registered game without a separate manual importer |
| M5 — Console preparation | Read-only inventory for the five added systems, then one platform/format/recipe at a time | [Per-platform gates](CONSOLE_PREPARATION.md) pass; updates/DLC never become separate playable base games; unknown inputs remain visible and blocked |
| M6 — Other managed setup and stores | Route emulator/port/runtime setup through manager incrementally; use shared jobs for supported Epic/GOG work | Existing isolation/rollback/saves preserved; official Steam remains a delegated client flow; truthful capability/progress and backend ownership |

M3 transport ownership and cutover are implemented. The manager hosts the existing
adapter, private/public engines, durable history and settings, and checkpoints
eligible jobs every second without a connected UI. Migration requires the library
lease, retains an exact private backup and original payload paths, and retires
older embedded writers before the daemon can restore jobs. Pause/Stop/settings
reset preserve completion/counters without status polling. CLI torrent controls,
metadata creation and persistent background seeding use IPC while a launcher owns
the library. Core alone publishes private-device roots and retries scan failures;
generic package completion never becomes an installation. See
[torrent migration, lifecycle and exact verification](TORRENTS.md).

M3 now links each completed supported archive to one explicit-platform review and
immutable provenance-bearing receipt. The manager rechecks its torrent pieces,
including neighboring-file boundary bytes, before inspection. Replays, changed
sources, cancelled reviews, failed registration, restart and source cleanup are
covered. A synthetic real-worker/UI flow reaches one registered Genesis game;
the owner's supplied 13.58 GB PS4 torrent completed receive-only with zero uploads
and was correctly blocked from package installation. See [exact scope](TORRENTS.md).

Broader large-transfer interruption/NAT, packaging and unsupported platform recipes
remain open. The next useful gate is a real supported new acquisition through
preparation and Play; M4 live provider acceptance can proceed independently.
Console package setup follows M5 one validated platform recipe at a time.
No universal installer is claimed.
Read-only console inventory has started ([observed files](CONSOLE_PREPARATION.md#observed-source-inventory));
console preparation recipes remain separate. Do not wait for every provider or
console to close one transport-to-preparation slice.
Store order remains Steam → compatibility/Epic → GOG after this manager foundation;
read-only Steam work need not wait. A separate management application, NAS mode,
remote API, universal installer and arbitrary recipe execution are deferred.

## Migration and verification checklist

- [ ] Snapshot library DB, transfer history, companion JSON, configuration and
  receipt formats into disposable migration fixtures; redact secrets. Inspect the
  next free schema version at implementation; do not reserve/reuse v4–v6.
- [ ] Adopt records with stable legacy-ID mappings and retained original paths.
  Unknown browser jobs and old unowned torrent folders require reconciliation,
  not automatic execution. Preserve missing-source history and corrupt files.
- [ ] Cut over one transport at a time: stop old owner, persist migration marker,
  import/verify new state, acknowledge cutover before new owner runs. Keep a backup
  and rollback procedure that cannot start both owners.
- [x] Inventory the supplied source tree read-only and exercise one small supported
  real archive in an isolated library. Originals and the existing user library
  remain unchanged; console packages/firmware/keys were not installed.
- [ ] Exercise staged-write crash recovery, failed receipt delivery/ack, manager
  restart, launcher closure, detached browser, missing drive, changed input,
  corrupt state, duplicate requests, cancellation and insufficient space.
- [ ] Test with synthetic fixtures first, then disposable data and one explicitly
  selected real source. Record exact runner/helper build, environment and live
  boot/input/save/exit/return evidence. Download/extraction alone closes no Play gate.
- [x] M1/M2 Rust/CLI/UI regressions, Clippy, native compile, worker/socket lifecycle,
  companion build and real browser close/reconnect were exercised. Installer
  distribution and additional transports retain their own acceptance. Later work
  must recheck the live schema; v6 is now allocated to preparation receipts.
