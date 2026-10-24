# Archive preparation and intake

Updated 2026-10-22. The M1 manager service now provides **durable, reviewed local
archive preparation**. The legacy immediate importer remains available below.
M2 now runs it in a separate hidden `marquee-manager` process; the launcher owns
library registration. See the [manager lifecycle/delivery contract](MARQUEE_MANAGER.md#implemented-worker-lifecycle-m2).

## Saved preparation workflow (implemented)

Library → Downloads and archives → add paths → choose a supported platform →
**Inspect selected archives** → review files/destination/space → **Approve and
prepare**. Inspection saves a review only; dropping/selecting files never extracts.
Preparation history reloads from the backend when the panel is reopened.

- The platform is explicit; no PS1/PS2 ISO size guess is used. Unknown platforms,
  installers and unsupported package members fail inspection without installing.
- `ArchivePlan` in `crates/media/src/archive.rs` shares bounded inspection/extraction with the legacy
  importer. Outputs preserve relative paths; known arcade sets remain intact ZIPs.
  CUE quote normalization is followed by actual track-manifest validation.
- Sources are always retained. SHA-256 binds the reviewed archive and is checked
  before/after extraction. A local hash is integrity evidence, not verified game identity.
- Each job owns `<data dir>/manager/installations/<job-id>` and a sibling private
  staging directory. Validate sizes, regular-file paths, CUE dependencies and output
  hashes, sync staged files/directories, persist the receipt, then promote the
  whole directory. Existing installations are never overwritten by this recipe.
- `manager/jobs.db` schema 4 persists jobs, reviews, attempts, receipts/outbox
  and `browser_jobs`; M1 schema 1 is backed up and adopted under exclusive
  ownership; schemas 2–3 are backed up before the provenance and browser-job
  upgrades. Manager IPC v5 is required for provenance-aware receipts, torrent
  authority and isolated PS3 firmware setup.
  `library.db` schema 6 adds only `preparation_receipts`. Root registration, scan
  and the library receipt record commit in one transaction, then the worker is acknowledged; failed publication
  rolls back and stays retryable. No existing library reset is required.
- Restart marks abandoned active jobs interrupted. Retry reconciles partial staging
  or an already prepared receipt. Registration retry does not extract again and
  can succeed after the source archive has moved. Lost acknowledgements reuse the
  committed edition IDs. Changed output files block publication.
- Cancel is supported before preparation, not during active extraction. Inspecting
  a cancelled source again explicitly returns it to review. Work continues when
  the panel or launcher closes. Completed receipts wait for Marquee to reconnect;
  interrupted jobs require explicit retry. Byte progress and automatic resume remain open.

Browser completions in manager mode have their own platform selector and
**Inspect downloaded archive** action. This rechecks completion-time bytes and
atomically links one preparation review to the browser job, without manually
adding archive paths. Review/approval remains explicit. Repeated inspection and
receipt delivery reuse the same identities. Provider selection, source hash and
attribution persist in the job and library receipt even after source cleanup.
Source availability is displayed separately; updates/DLC need their own recipe.

CLI, over the same service:

```sh
marquee prepare inspect "/absolute/path/Game.zip" --system genesis
marquee prepare list                    # reconcile with the library
marquee prepare start JOB_ID --review-token TOKEN_FROM_INSPECTION  # accept and detach
marquee prepare run JOB_ID --review-token TOKEN_FROM_INSPECTION
marquee prepare cancel JOB_ID
```

`start`/`inspect` and `manager status|start|stop` work without taking the library
lease. `run` waits for completion and exits nonzero if preparation/publication fails;
`list` registers pending receipts. Close another library owner before those two
library-writing CLI commands. The UI reconciles automatically while active.
API: `preparation_systems`, `inspect_preparation` (`path`, `system`),
`preparation_jobs`, `start_preparation` (`id`, `review_token`), and
`cancel_preparation` (`id`). The start command returns after claiming a worker;
desktop and HTTP share the same dispatch. Concurrent preparation is refused;
status reads cannot rewrite a live worker's newer state.

### Verification and remaining limits

Synthetic ZIP/7z/RAR tests cover review boundaries, source preservation, v5→v6
migration with personal entries intact, interruptions around staging/promotion,
failed publication/rollback, lost acknowledgement, duplicate actions, corrupt
state, unsupported packages, changed source/output, unsafe paths and CUE failures.
Space-check tests refuse insufficient or unknown capacity before staging.

Production browser checks used the owner's Sonic Genesis ZIP with isolated
libraries: inspect → review → prepare → Library entry → reload history; M2 also
closed the launcher before registration, observed independent completion, then
reopened and registered once. Worker UI start/stop and M1 pilot adoption passed.
The original SHA-256 was unchanged and the same job had one attempt. The rendered
panel was inspected. This is preparation/registration evidence, not a game boot,
controller/save check or native Tauri picker/drop acceptance. Existing emulator
prerequisite and launch gates remain open. Five [added console platforms](CONSOLE_PREPARATION.md)
remain outside this recipe; do not use legacy ISO guessing for their packages.

## Legacy immediate importer

Turns a downloaded `.zip`, `.7z` or `.rar` into library files. `marquee intake <archive-or-folder>...` inspects each archive, picks the system from its contents, unpacks into that system's library folder and rescans. Core API: `Core::intake_archive`; JSON command `intake_archive` (`path`, `delete_source`). Code: `crates/core/src/intake.rs`.

```sh
marquee intake ~/Downloads                 # every .zip/.7z/.rar in the folder
marquee intake "Game (USA).7z" --delete    # remove the archive once verified
marquee intake x.zip --no-scan             # skip the rescan
```

### Legacy desktop and browser UI

Library → Add games → Import ZIP/7z/RAR archives opens the importer. A separate
Library button reopens progress and results. Desktop can select multiple
archives or drop them onto Library; dropping only queues them for review.
Browser users enter absolute paths on the Marquee computer, with no browser
upload or assumed local-file path. Folder import remains separate.

Batches contain up to 50 unique archives, processed sequentially. Each archive
reports its detected system, destination, outcome, filenames and retained or
removed source. A failed archive does not hide later results. Successful and
AlreadyPresent outcomes trigger one real scan and snapshot refresh after the
batch. Scan failures remain separate from extraction, with a retry that does
not re-extract successful archives. Original archives are kept unless source
removal is explicitly selected for that batch; the choice resets afterwards.

The UI shows archive-count and scan phases, not fabricated byte percentages.
It can be closed while work continues in that window. Byte progress,
cancellation, watched inboxes and restart-persistent import jobs are not yet
implemented. Native drop/file-picker and rendered interaction still need live
acceptance; tests cover shared dispatch and batch failure/recovery behavior.

### Legacy system detection

| Contents | Result |
| --- | --- |
| One cartridge image (`.nes`, `.sfc`, `.gb`, `.gbc`, `.pce`, `.z64`, `.nds`, ...) | File goes straight into that system's root |
| `.cue`/`.chd`/`.pbp`, or `.bin` with a `.cue` | PS1; several files get a folder named after the archive |
| `.iso` | PS2 if at least 750 MiB, otherwise PS1 (a size guess; check the result) |
| `.md`/`.gen`/`.smd` | Extracted into a registered Genesis root; launched through managed BlastEm |
| ZIP with `.rom` entries or `NNN-` MAME-style names | Arcade (Neo Geo) set copied **unchanged** next to the other sets |
| Only `.bin` (BIOS), or only text/images | Skipped with a hint; use `marquee bios import` for BIOS |
| Files for more than one system | Skipped; unpack by hand |

The CLI processes the whole archive batch and scans successful imports even if
another archive fails. If any archive import fails, it exits nonzero with an
`INTAKE_FAILED` summary; successful files are kept. `--no-scan` suppresses the
scan without hiding import failures.

The CLI rescans `AlreadyPresent` results too: files unpacked before their adapter existed can enter the library once support is added. `--no-scan` still suppresses this. The core/JSON intake call itself does not scan; API callers must request a scan.

CUE files with provider-style single-quoted `FILE` names are normalized to
standard double quotes in the imported copy, preserving names and line endings.
Re-import also repairs a same-size CUE extracted by the older importer. The
archive remains unchanged. Missing or mismatched BIN names are not guessed: all
referenced tracks must actually exist. In the current XS Moto archive, the CUE
and BIN filenames differ independently of the quoting problem, so quote repair
alone does not make that archive playable.

Documents and images in the archive (`.txt`, `.nfo`, `.jpg`, ...) are ignored. A system with no library folder gets one next to your existing roots and is registered.

### Legacy safety

- Entries must stay inside the destination: `..`, absolute paths and symlinks refuse the whole archive before anything is written.
- At most 20,000 entries and 128 GiB unpacked; an entry may not write more bytes than it declares.
- Free disk space (plus 256 MiB) is checked first.
- Files are unpacked into a hidden staging folder and moved into place afterwards, so a failure leaves no partial game.
- Existing files with the same name and size are left alone (`AlreadyPresent`). A file of at least 64 MiB with the same size and extension as something already in the folder produces a **possible duplicate** warning, never a skip or delete.
- `--delete` removes the archive only after every unpacked file is verified by size.

### Legacy limits

- RAR4/RAR5 link and redirection entries (including hardlinks and file copies) are rejected from packed native headers. UnRAR only decompresses bytes through a bounded callback; Marquee creates regular staging files itself, checks CRC errors and refuses short/oversized output. Synthetic RAR fixtures cover both versions and hostile links/paths.
- `.rar` is read through the bundled UnRAR library (extraction only; encrypted headers/files and multi-volume sets are refused); the format is detected by content, not file name.
- Desktop drop/file picking and browser host-path UI are implemented. Watched inbox folders and restart-persistent jobs remain open.
- System detection is by extension and size; it does not read the disc image to tell PS1 from PS2, and does not verify ROM hashes.
- Whether a Neo Geo set actually runs depends on its BIOS and parent sets (see `neogeo.rs`).
