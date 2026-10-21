# Archive intake

Turns a downloaded `.zip` or `.7z` into library files. `marquee intake <archive-or-folder>...` inspects each archive, picks the system from its contents, unpacks into that system's library folder and rescans. Core API: `Core::intake_archive`; JSON command `intake_archive` (`path`, `delete_source`). Code: `crates/core/src/intake.rs`.

```sh
marquee intake ~/Downloads                 # every .zip/.7z in the folder
marquee intake "Game (USA).7z" --delete    # remove the archive once verified
marquee intake x.zip --no-scan             # skip the rescan
```

## What it decides

| Contents | Result |
| --- | --- |
| One cartridge image (`.nes`, `.sfc`, `.gb`, `.gbc`, `.pce`, `.z64`, `.nds`, ...) | File goes straight into that system's root |
| `.cue`/`.chd`/`.pbp`, or `.bin` with a `.cue` | PS1; several files get a folder named after the archive |
| `.iso` | PS2 if at least 750 MiB, otherwise PS1 (a size guess; check the result) |
| `.md`/`.gen`/`.smd` | Stored under `roms/genesis`; Marquee has no Genesis emulator yet |
| ZIP with `.rom` entries or `NNN-` MAME-style names | Arcade (Neo Geo) set copied **unchanged** next to the other sets |
| Only `.bin` (BIOS), or only text/images | Skipped with a hint; use `marquee bios import` for BIOS |
| Files for more than one system | Skipped; unpack by hand |

Documents and images in the archive (`.txt`, `.nfo`, `.jpg`, ...) are ignored. A system with no library folder gets one next to your existing roots and is registered.

## Safety

- Entries must stay inside the destination: `..`, absolute paths and symlinks refuse the whole archive before anything is written.
- At most 20,000 entries and 128 GiB unpacked; an entry may not write more bytes than it declares.
- Free disk space (plus 256 MiB) is checked first.
- Files are unpacked into a hidden staging folder and moved into place afterwards, so a failure leaves no partial game.
- Existing files with the same name and size are left alone (`AlreadyPresent`). A file of at least 64 MiB with the same size and extension as something already in the folder produces a **possible duplicate** warning, never a skip or delete.
- `--delete` removes the archive only after every unpacked file is verified by size.

## Limits and not done

- `.rar` is not supported (needs a native library); the format is detected by content, not file name.
- No UI yet: no drop target or watched inbox folder.
- System detection is by extension and size; it does not read the disc image to tell PS1 from PS2, and does not verify ROM hashes.
- Whether a Neo Geo set actually runs depends on its BIOS and parent sets (see `neogeo.rs`).
