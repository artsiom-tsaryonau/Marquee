# Archive intake

Turns a downloaded `.zip`, `.7z` or `.rar` into library files. `marquee intake <archive-or-folder>...` inspects each archive, picks the system from its contents, unpacks into that system's library folder and rescans. Core API: `Core::intake_archive`; JSON command `intake_archive` (`path`, `delete_source`). Code: `crates/core/src/intake.rs`.

```sh
marquee intake ~/Downloads                 # every .zip/.7z/.rar in the folder
marquee intake "Game (USA).7z" --delete    # remove the archive once verified
marquee intake x.zip --no-scan             # skip the rescan
```

## Desktop and browser UI

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

## What it decides

| Contents | Result |
| --- | --- |
| One cartridge image (`.nes`, `.sfc`, `.gb`, `.gbc`, `.pce`, `.z64`, `.nds`, ...) | File goes straight into that system's root |
| `.cue`/`.chd`/`.pbp`, or `.bin` with a `.cue` | PS1; several files get a folder named after the archive |
| `.iso` | PS2 if at least 750 MiB, otherwise PS1 (a size guess; check the result) |
| `.md`/`.gen`/`.smd` | Extracted into a registered Genesis root; launched through managed BlastEm |
| ZIP with `.rom` entries or `NNN-` MAME-style names | Arcade (Neo Geo) set copied **unchanged** next to the other sets |
| Only `.bin` (BIOS), or only text/images | Skipped with a hint; use `marquee bios import` for BIOS |
| Files for more than one system | Skipped; unpack by hand |

The CLI rescans `AlreadyPresent` results too: files unpacked before their adapter existed can enter the library once support is added. `--no-scan` still suppresses this. The core/JSON intake call itself does not scan; API callers must request a scan.

Documents and images in the archive (`.txt`, `.nfo`, `.jpg`, ...) are ignored. A system with no library folder gets one next to your existing roots and is registered.

## Safety

- Entries must stay inside the destination: `..`, absolute paths and symlinks refuse the whole archive before anything is written.
- At most 20,000 entries and 128 GiB unpacked; an entry may not write more bytes than it declares.
- Free disk space (plus 256 MiB) is checked first.
- Files are unpacked into a hidden staging folder and moved into place afterwards, so a failure leaves no partial game.
- Existing files with the same name and size are left alone (`AlreadyPresent`). A file of at least 64 MiB with the same size and extension as something already in the folder produces a **possible duplicate** warning, never a skip or delete.
- `--delete` removes the archive only after every unpacked file is verified by size.

## Limits and not done

- RAR4/RAR5 link and redirection entries (including hardlinks and file copies) are rejected from packed native headers. UnRAR only decompresses bytes through a bounded callback; Marquee creates regular staging files itself, checks CRC errors and refuses short/oversized output. Synthetic RAR fixtures cover both versions and hostile links/paths.
- `.rar` is read through the bundled UnRAR library (extraction only; encrypted headers/files and multi-volume sets are refused); the format is detected by content, not file name.
- Desktop drop/file picking and browser host-path UI are implemented. Watched inbox folders and restart-persistent jobs remain open.
- System detection is by extension and size; it does not read the disc image to tell PS1 from PS2, and does not verify ROM hashes.
- Whether a Neo Geo set actually runs depends on its BIOS and parent sets (see `neogeo.rs`).
