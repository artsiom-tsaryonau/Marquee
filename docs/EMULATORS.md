# Managed emulators and BIOS

Updated 2026-10-21. Marquee installs emulators and holds BIOS images inside its own data folder, so everything can be removed on demand. Package managers (Flatpak and others) are an advanced alternative, never the default.

## Current catalogue and integration boundary

| Emulator | Platform | Current Marquee capability |
| --- | --- | --- |
| DuckStation | PS1 | Managed lifecycle plus library-disc launch adapter |
| PPSSPP | PSP | Managed lifecycle and standalone opening |
| RPCS3 | PS3 | Managed lifecycle and standalone opening |
| Flycast | Dreamcast | Managed lifecycle and standalone opening |
| Azahar | Nintendo 3DS | Managed lifecycle and standalone opening |
| Cemu | Wii U | Managed lifecycle and standalone opening |
| xemu | Original Xbox | Managed lifecycle and standalone opening |

PCSX2 remains integrated through its dedicated installer. A catalogue entry is implementation of acquisition/opening, not proof of a successful current upstream install, complete configuration isolation, firmware readiness or a playable game. Install requires a matching upstream AppImage with a published checksum; missing or changed packaging must produce an error. Additional platforms still need system descriptors, media import, launch flags, prerequisite checks, save behavior and live acceptance before their games appear as playable library entries. Standalone emulator lifetime is not recorded as game playtime.

## What is needed to install and manage DuckStation

Verified against the upstream release on 2026-10-21.

| Need | Finding |
| --- | --- |
| Download source | GitHub release of `stenzek/duckstation`. No key or login. |
| Version identity | The release tag is always `latest`, so the tag cannot detect updates. Marquee uses the asset's SHA-256 (shown as `latest-<first 8 hex>`). |
| Asset | `DuckStation-x64.AppImage` (the `SSE2` build is excluded; ARM builds do not match). About 93 MB. |
| Integrity | GitHub publishes a `sha256:` digest for the asset. Marquee verifies it and **refuses to install** an emulator whose release publishes none. |
| Running without FUSE | The AppImage is unpacked with `--appimage-extract` into `runners/duckstation/current` (Fedora has no FUSE 2 by default). The launcher is `AppRun`. |
| Isolation | Setting `XDG_CONFIG_HOME` is enough: DuckStation puts its whole user folder (settings, BIOS, memory cards, save states, shaders) in `$XDG_CONFIG_HOME/duckstation`. Nothing was written to `~/.config` or `~/.local/share` in the check. |
| BIOS | DuckStation searches `duckstation/bios` under that folder and picks the image matching the disc's region itself. Marquee only has to put the files there. |
| First run | DuckStation shows its own setup wizard once (`SetupWizardIncomplete`). Marquee does not pre-create its settings file, because a half-written one risks a reset. |
| Launch flags | `-batch -fullscreen -nogui <image>` (already used by the launcher code). |

## What the catalogue contains

| Emulator | Systems | Library games launch through it | Verified |
| --- | --- | --- | --- |
| DuckStation | PS1 | yes | Installed, unpacked, started under isolation |
| Flycast | Dreamcast | no, opens its own window | Installed, unpacked, started under isolation |
| Cemu | Wii U | no | same |
| RPCS3 | PS3 | no | same |
| PPSSPP | PSP | no | same |
| xemu | Xbox | no | same |
| Azahar | 3DS | no | same |

"Verified" means: the release was downloaded, matched its published checksum, unpacked, and started with its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_CACHE_HOME`; nothing appeared in `~/.config`, `~/.local/share` or `~/.cache`. It does **not** mean a game was played. Only emulators with a runner kind (DuckStation) register as library runners; the rest can be installed, updated, opened and removed, and their systems have no library scanning yet.

Left out on purpose: **mGBA** (its releases publish no checksum, so Marquee would refuse it), **melonDS** (ships a zip around the AppImage, which needs zip support), **Dolphin** (no official Linux AppImage), **shadPS4** (zip, early-stage).

## Layout (all under `~/.local/share/marquee`)

| Path | Holds | Removed by |
| --- | --- | --- |
| `runners/<emulator>/current` | The unpacked program | `marquee emulators uninstall <id>` |
| `<emulator>/xdg-config/` | Settings, saves, memory cards, BIOS | `uninstall <id> --purge-data` |
| `pcsx2/xdg-config/PCSX2/bios` | PS2 BIOS (PCSX2 keeps its own tree here) | `marquee bios remove ps2` |
| `duckstation/xdg-config/duckstation/bios` | PS1 BIOS | `marquee bios remove ps1` |

Updating replaces only `runners/<emulator>/current`, after the new build is downloaded, verified and unpacked in a staging folder. A failed update leaves the working install alone. Uninstalling the program never deletes saves unless asked.

## Commands

```
marquee emulators list | check | install <id> | uninstall <id> [--purge-data]
marquee bios list | import <folder or file> | remove ps1|ps2
```

The same operations are API commands: `list_emulators`, `check_emulator_updates`, `install_emulator`, `uninstall_emulator`, `list_bios`, `import_bios`, `remove_bios`. **Tools → Emulators** shows the same: install, update, open, remove (with a separate, confirmed "remove with saves"), a copyable package-manager alternative, and a BIOS panel (import a folder, per-region listing, confirmed removal). `open_emulator` / `marquee emulators open <id>` starts an emulator's own window.

## BIOS

- Marquee never downloads BIOS images. You supply them.
- `bios import` reads each file's contents to identify it (a PS1 `Version x.y dd/mm/yy <A|E|J>` line, a PS2 `ROMVER` record), so odd filenames work. Unrecognised files are not copied. PS2 NVRAM/MEC/ROM sidecars of a recognised image are copied with it.
- **PS2 region matching:** before launching with Marquee's PCSX2, Marquee picks the newest BIOS of the game's region and writes it to `[Filenames] BIOS` in `PCSX2.ini`. This only happens once PCSX2 has written that file itself, and the key name is unverified against a live launch. With no match, PCSX2 keeps its own choice.
- **PS1 region matching** is done by DuckStation itself from the BIOS folder.

## Package managers (advanced)

Each catalogue entry may list `packages`: a manager and the command you could run yourself (for DuckStation: `flatpak install --user flathub org.duckstation.DuckStation`). `list_emulators` returns them as `alternatives` so a screen can show a copyable suggestion. Marquee never runs them. It still detects a Flatpak or native copy as a separate runner, but prefers its own managed one.

## Adding another emulator

1. Add `catalog/emulators/<id>.json` (copy `duckstation.json`): GitHub repo, asset suffix and excludes, `rolling` if the tag never changes, the launcher path inside the unpacked AppImage, the user-data folder name, optional BIOS folder and package hints.
2. List the file in `CATALOGUE` in `crates/core/src/emulators.rs`.
3. If it is a new emulator family, add a `RunnerKind` in `model.rs`, map its systems in `runners::preferred_kind`, and add its launch flags to `runners::launch_argv`.

Install, update checks, uninstall, runner registration and isolation then come from the catalogue. Limits today: AppImage releases only (no zip/tar emulators yet), and the emulator must honour `XDG_CONFIG_HOME` for its user folder. PCSX2 still uses its older dedicated installer (`tools.rs`); moving it onto the catalogue is a follow-up.

## Not done

- Emulators other than DuckStation do not launch library games; their systems cannot be scanned yet.
- A live DuckStation Play, quit and duration check has not been run. See [PS1](PS1.md).
- Ports still have no uninstall.
- PCSX2 is not yet on the catalogue.
