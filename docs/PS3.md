# PS3 integration

Updated 2026-10-25. **Discovery, reviewed firmware setup and verified ISO library registration; PS3 Library Play is not implemented.**
RPCS3 remains a managed standalone tool. This page records the staged PS3 work
under [console preparation](CONSOLE_PREPARATION.md).

## P3-0 — Read-only discovery (implemented)

Tools → RPCS3 → Configure → **PS3 content discovery** inspects one selected file
or collection folder. The native dialog uses file/folder pickers; the browser
accepts a path on the Marquee host. The shared API is `inspect_ps3` with `path`.
The CLI works without opening a library, taking its lease, restoring transfers,
creating a data directory or starting RPCS3:

```sh
marquee ps3-inspect /absolute/path/to/content
```

Behavior:

- ZIP/7z/RAR: use shared bounded archive-header enumeration. Report declared
  members and unpacked size without extracting or hashing payloads. ISO, PKG,
  firmware and archived metadata names are **candidates**, not verified identity,
  platform, decrypted content or playable installations. Password/corrupt/unsafe
  header errors remain errors. Encrypted payload readiness is not inferred from
  a successful header listing.
- Folders: inspect `PS3_GAME/PARAM.SFO` or `PARAM.SFO`. The original bounded PSF
  parser validates tables, lengths, formats, duplicate keys, UTF-8 text and title
  IDs, retaining TITLE, TITLE_ID, CATEGORY, APP_VER and PS3_SYSTEM_VER.
- DG is a disc-folder candidate; HG a digital-game folder candidate. GD is retained
  as game data/update candidate, never a base game. Other categories stay other
  content. Metadata does not prove authenticity or compatibility. Missing PS3
  system-version evidence remains explicit.
- Record regular `USRDIR/EBOOT.BIN`, `PS3_DISC.SFB` and `ICON0.PNG` presence without
  executing, parsing the boot file, verifying the disc manifest or decoding art.
  Select the disc root to include its manifest in the inspected scope. Missing
  layout files block a complete-layout status. Even a complete-looking layout
  still requires firmware, save-path and launch validation.
- Folder scanning reports archives as separate candidates; select an archive
  directly to inspect its headers. It does not recursively open every archive.
- Bounds: 1 MiB per SFO, 512 SFO entries, folder depth 8, 20,000 directory entries,
  200 candidates and 100 diagnostics. Folder symlinks are skipped; metadata and
  known-layout paths containing symlinks are refused. Devices and relative input
  paths are rejected. Limits are reported rather than silently claiming a full
  inventory. This is a filesystem snapshot, not a race-proof sandbox.

The inspector does not register games, create library roots, cache artwork,
install firmware, read license/key contents, decrypt or modify source files.
Every report has `playable: false`. Report sizes are declared sizes, not a disk
space estimate or a cryptographic integrity check.

### Actual input evidence

The supplied God of War III 7z was enumerated read-only:

- Outer archive: 35,786,199,527 bytes.
- One `God of War III (USA, Canada) (v02.00).dec.iso` member: 38,169,870,336 bytes.
- One text member: 324 bytes.
- No PARAM.SFO was read, no title ID resolved and no decryption status established.
  The `.dec.iso` spelling is not verification.

The ignored local JSON receipt is
`data/manager-pilot/ps3-discovery-20261024.json`. The CLI was invoked with an unusable
`--data-dir /dev/null/ps3-read-only-probe` to verify that inspection bypasses data
resolution/library ownership while the desktop app is running. No full archive
hash or extraction was performed.

### Format references

Reviewed RPCS3 revision: `46b7fb670b280d08036c7670270b713daef912c2`.
The implementation is original Rust; upstream source was used to check format
facts and layout behavior, not copied into Marquee:

- [PSF field layout and validation](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/Loader/PSF.cpp)
- [PSF field formats](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/Loader/PSF.h)
- [Disc/title boot and inventory behavior](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/Emu/System.cpp)

## P3-1 — Firmware and runner readiness (implemented; live acceptance recorded below)

Tools → RPCS3 → Configure → **PS3 firmware setup** selects and reviews a supplied
PUP, then starts a manager-owned installation. A stored PUP alone never counts as
installed firmware. CLI equivalents:

```sh
marquee ps3-firmware status
marquee ps3-firmware review /absolute/path/to/PS3UPDAT.PUP
marquee ps3-firmware start <review_token>
```

The review records structural PUP validation, version, size, SHA-256, installed
runner version and program fingerprint. It does not claim publisher authenticity;
RPCS3 verifies the package contents. Start rechecks the program and an isolated
source snapshot, requires 2.5 GiB free, and runs the pinned RPCS3 `--installfw`
recipe in a fresh HOME/XDG profile. Existing nonempty firmware and custom `vfs.yml`
are refused rather than replaced; there is no upgrade/replace path yet. The
supplied original is retained. If the manager is down with a leftover
running/promoting job, status and RPCS3 open/update/removal persist that job as
interrupted (or recover a verified promotion) before treating the runner as idle.

RPCS3's informational welcome is hidden in this setup profile to avoid competing
modal startup/install dialogs. Firmware confirmation remains interactive. After
RPCS3 exits, its saved welcome opt-out is carried into the regular managed profile
without copying other GUI preferences, filesystem settings, saves or caches.
A later rejected welcome preference is not copied as an opt-out. Standalone Tools
opening still opens the emulator; future Library Play should use `--no-gui` with
a validated boot target, optionally `--fullscreen`. Hiding the main window is not
headless rendering. `--no-gui` does not support firmware installation.

The manager requires all of: clean exit, the exact version-specific successful
installation log, valid installed version/SELF markers and a bounded regular-file
output manifest. Only `dev_flash` is published with a no-replace rename; no user
saves/settings are replaced. Output hashes and a private receipt are recorded.
Interrupted jobs are not automatically rerun. A crash after verified promotion
can recover its receipt from a matching full manifest. Accepted setup survives a
launcher/client disconnect; manager Stop drains it. RPCS3 open/update/removal is
blocked while its firmware job is active. The current cross-process mutation
limits of the manager still apply; do not run external RPCS3 writers concurrently.

Private state: `<data>/manager/ps3-firmware/{job.json,jobs/,receipt.json,work/}`.
Installed firmware: `<data>/rpcs3/xdg-config/rpcs3/dev_flash/`. Work profiles retain
logs and snapshots for diagnosis; no automatic source cleanup is performed.

### Pinned build and actual input evidence

- RPCS3 `v0.0.43-20395-46b7fb67`, release
  `build-46b7fb670b280d08036c7670270b713daef912c2`.
- Linux AppImage SHA-256:
  `9df18bd0af25e94c5a03936b04b3e5a332a5c22e6fa61e6ab93376b027817a5c`.
- Supplied PUP: version **4.93**, 206,197,916 bytes, SHA-256
  `158471fd834f8ea8036136b6aab43cd86c7ba73d79ca30e0af3c0fe0001cf365`.
- The isolated interactive pilot exited 0, logged successful 4.93 installation and
  produced valid version, `vsh.self` and `liblv2.sprx` markers.
- Two `--headless --installfw` probes extracted firmware but aborted during
  RPCS3 teardown (exit 134). That path is deliberately not accepted as success.
- An initial main-profile job exited 0 after canceling installation. It correctly
  failed verification and published nothing. A second attempt installed into
  staging but had an unresponsive welcome; it was stopped without publication.
  The revised recipe hides that informational welcome before starting RPCS3.
- Revised main-profile job completed: clean exit 0, verified firmware 4.93 and
  no-replace publication with a managed receipt. The original PUP hash was
  unchanged. The regular managed profile retains `infoBoxEnabledWelcome=false`.
  Output: 1,075 files, 195,078,808 bytes; manifest SHA-256
  `8d4551ecda94894db68c12f7551d62dbed2fb35e4cc97a9c7a280ce55688347e`.
  Firmware PPU compilation completed; this was not a game boot test.

Validation includes malformed/linked PUPs, changed source/program reviews,
existing-destination protection, false-success/nonzero installer output, saved GUI
preference preservation, interrupted promotion recovery and an actual detached
manager/client-disconnect/draining-Stop test. Live game compatibility is separate.

Upstream behavior checked at the pinned revision:
[CLI modes](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/rpcs3.cpp),
[firmware installer](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/rpcs3qt/main_window.cpp),
[welcome behavior](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/rpcs3/rpcs3qt/welcome_dialog.cpp),
[Linux XDG paths](https://github.com/RPCS3/rpcs3/blob/46b7fb670b280d08036c7670270b713daef912c2/Utilities/File.cpp).

## License and disc-key inputs (assessed; import/decryption not implemented)

Archive headers only were inspected: `exdata.zip` contains 28,798 RAP, 662 EDAT,
4 BIN and 2 RIF entries; the disc-key ZIP contains 4,366 DKEY entries. Key payloads
were not opened, extracted or committed. Local license/key formats are ignored
by Git, in addition to the existing archive/PUP exclusions.

RAP/RIF relate to digital-content licenses; disc keys relate to encrypted disc
images. Their presence does not prove a matching title, license readiness or that
an image needs decryption. The `.dec.iso` filename remains unverified. Check the
pinned RPCS3 build's native key/image support before adding a decryptor.

[PS3 Quick Disc Decryptor](https://github.com/ElektroStudios/PS3-Quick-Disc-Decryptor)
was reviewed as a reference: it is an Apache-2.0 Windows .NET desktop wrapper for
PS3Dec, with ISO/ZIP and DKEY inputs. Its bundled engine needs separate licensing
review. No code or keys were copied. A native Linux manager recipe is preferred
if the selected input needs preparation.

## Verified ISO registration (implemented)

RPCS3 content discovery now offers **Add disc to Games** for an unpacked ISO.
The shared API is `import_ps3_disc` with an absolute `path`. A PS3 library folder
can also be scanned. Registration reads bounded ISO9660 directories and validates
base-disc PARAM.SFO metadata, a readable SELF header and the disc manifest header.
It records a `ps3_iso` media entry, SFO title and title ID. Repeated import of the
same path preserves the edition ID. Update-only inputs and unreadable executable
headers are rejected before creating an entry.

This creates an actual library entry attached to the ISO, not a wishlist entry.
Media collections link to its edition ID; game details have **Overview**, **Media**
and **Metadata**, with Soundtrack / Images / Videos sections inside Media. Games
can therefore own extras before their launch adapter is accepted. **Launch
pending** remains disabled, and the backend rejects PS3 launch explicitly. PCSX2
and fake runners are never advertised as PS3-compatible. The shared archive
preparation picker/recipe still excludes PS3; this slice does not add PKG, digital
license import, decryption or a universal PS3 archive workflow.

The owner's GoW3 ISO was promoted by same-filesystem no-replace rename from its
pilot folder into `<data>/roms/ps3/`, preserving its 38,169,870,336 bytes and the
original archive. Actual inspection resolved **God of War III / BCUS98111**, disc
category DG, app version 01.00 and minimum firmware 04.2100. It links to the existing
**God of War III — Disc extras** collection. Search against SteamGridDB returned
the original game and Remastered as separate artwork identities; explicit review
retains the original title/platform and does not merge editions.

Main-library acceptance on 2026-10-25 registered one BCUS98111 entry, retained
that ID across repeated import/rescan, linked all four existing extras and applied
the reviewed original-game artwork. The owner confirmed the updated game/Media
layout works. No RPCS3 game process was launched during this registration step.

## Remaining steps

1. **P3-2 — One real preparation path.** This collection needs a reviewed archive
   → disc-image path. Verify the pinned RPCS3 build's accepted filesystem/image
   formats and encryption requirements before choosing direct ISO or a prepared
   folder. Inspect title identity after preparation; reject unsupported content.
   Use manager jobs, space checks, recovery and receipts. No automatic 38 GB
   extraction was started by discovery or firmware setup.
2. **P3-3 — Library launch and ownership (registration implemented).** PS3
   descriptors, verified ISO entries and Media links are implemented. Add launch
   ownership, edition/version relationships and isolated user saves.
   Launch directly with `--no-gui`; keep setup/configuration a separate action.
   Track the game process and exit; never mark an update/extra as a base title.
3. **P3-4 — Live game acceptance.** Observe boot/video/audio, keyboard mappings,
   native saves across relaunch and clean exit/return. Report the exact game/build.
   Streaming follows local acceptance; physical-controller testing waits for
   hardware. A heavy title may require a smaller pilot to distinguish adapter
   failures from game-specific compatibility.
