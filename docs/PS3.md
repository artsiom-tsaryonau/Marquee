# PS3 integration

Updated 2026-10-24. **Discovery only; PS3 Library Play is not implemented.**
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

## Remaining steps

1. **P3-1 — Firmware and runner readiness.** Pin/record the installed RPCS3 build;
   validate firmware acquisition/import and installation in its managed profile.
   Storing a PUP is not successful installation. Preserve existing RPCS3 data.
2. **P3-2 — One real preparation path.** This collection needs a reviewed archive
   → disc-image path. Verify the pinned RPCS3 build's accepted filesystem/image
   formats and encryption requirements before choosing direct ISO or a prepared
   folder. Inspect title identity after preparation; reject unsupported content.
   Use manager jobs, space checks, recovery and receipts. No automatic 38 GB
   extraction was started by discovery.
3. **P3-3 — Library launch and ownership.** Add PS3 media/system descriptors,
   validated boot targets, edition/version relationships and isolated user saves.
   Track the game process and exit; never mark an update/extra as a base title.
4. **P3-4 — Live game acceptance.** Observe boot/video/audio, keyboard mappings,
   native saves across relaunch and clean exit/return. Report the exact game/build.
   Streaming follows local acceptance; physical-controller testing waits for
   hardware. A heavy title may require a smaller pilot to distinguish adapter
   failures from game-specific compatibility.
