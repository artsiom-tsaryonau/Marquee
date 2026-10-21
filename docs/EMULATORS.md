# Managed emulators and BIOS

Updated 2026-10-21. Marquee installs emulators and holds BIOS images inside its own data folder, so everything can be removed on demand. **Policy (2026-10-21):** Marquee installs, configures and runs emulators itself, from archives named in catalogue files. It does not use Flatpak or other package managers, and it no longer discovers host copies of emulators. Setup that an emulator would normally ask the user for is written by Marquee before launch.

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
| mGBA | GB / GBC / GBA | Opt-in pinned source-build pilot, Qt/SDL standalone tools; no library adapter |

PCSX2 is a normal catalogue entry (`catalog/emulators/pcsx2.json`, runner `runner.pcsx2.marquee`): the same verified-download-and-unpack path as every other emulator. The old dedicated installer (`tools.rs`) is gone. An install made by it is adopted as-is on the next refresh (recorded as version `adopted`), and its old `runner.pcsx2.unpacked` row is disabled, so nothing is downloaded twice (checked on this host). The install command `install_pcsx2` remains for the install dialog and delegates to the catalogue. A catalogue entry is implementation of acquisition/opening, not proof of a successful current upstream install, complete configuration isolation, firmware readiness or a playable game. Install requires a matching upstream AppImage with a published checksum; missing or changed packaging must produce an error. Additional platforms still need system descriptors, media import, launch flags, prerequisite checks, save behavior and live acceptance before their games appear as playable library entries. Standalone emulator lifetime is not recorded as game playtime.

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
| First run | Before each library launch Marquee sets `[Main] SetupWizardIncomplete = false` in `duckstation/settings.ini` (creating the file if absent) and touches no other key; DuckStation fills its own defaults. Checked on this host: the flag went `true` → `false` and DuckStation started. |
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

mGBA's published binary remains excluded by the checksum policy; it now has a separate [managed source-build pilot](#source-builds-mgba-pilot). **melonDS** still needs support for a ZIP-wrapped AppImage, **Dolphin** needs a different acquisition recipe, and **shadPS4** is a ZIP-packaged experimental follow-up. Building a program does not add its systems to library import.

See [game-only windows versus displayless execution](#game-only-windows-and-headless-emulation). PCSX2 library launches now include `-nogui`; DuckStation already did. Both still show the actual game window.

## Layout (binary recipes under `~/.local/share/marquee`)

The source-build pilot additionally uses rootless Podman's shared image storage; see its [storage and runtime boundary](#storage-and-runtime-boundary).

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

- Marquee downloads and unpacks emulator binaries. **Target:** firmware acquisition jobs from authorized sources (same job UI as store downloads). **Today:** import a folder or file via Tools, CLI, or API; Marquee identifies regions and stores copies locally. The app does not ship BIOS files in the installer.
- `bios import` reads each file's contents to identify it (a PS1 `Version x.y dd/mm/yy <A|E|J>` line, a PS2 `ROMVER` record), so odd filenames work. Unrecognised files are not copied. PS2 NVRAM/MEC/ROM sidecars of a recognised image are copied with it.
- **PS2 first run and region matching:** before launching with Marquee's PCSX2, Marquee picks the newest BIOS of the game's region and writes `[Filenames] BIOS`. If `PCSX2.ini` does not exist yet it writes the smallest file PCSX2 accepts (`[UI] SettingsVersion = 1`, `SetupWizardIncomplete = false`, the BIOS), so the wizard never opens. An existing file keeps all its other settings. With no usable BIOS nothing is written. Verified live on this host with Vice City (USA) and `SCPH-70012.bin`.
- **BIOS inbox:** any recognised BIOS file or `.zip` placed in `<data dir>/bios-inbox` (plus an optional folder in the `bios_inbox` setting) is imported at the next launch, without a click. Already-present files are skipped. Marquee still never downloads BIOS files. See [RetroBIOS notes](#retrobios-reference) for what other emulators will need.
- **PS1 region matching** is done by DuckStation itself from the BIOS folder.

## Tools screen

Emulators appear as picture cards in the Library's style: platform chips (from each emulator's systems), Installed/Updates chips, and the search box. The picture is the emulator's own icon, copied out of the unpacked AppImage into the art cache the first time it is listed, over generated art. Cards open a dialog with status, install/update/open, firmware status, install locations, and remove with or without saves. The per-emulator launch-argument editor and the "Open setup" buttons were removed from the UI (2026-10-21): Marquee writes each emulator's first-run settings itself. The backend commands (`configure_emulator`, `open_runner_setup`) still exist.

Saved launch arguments (`emulator.<id>.config`, at most 16 values of 512 characters, no line breaks) are still honoured by both Open and library launches and used internally by the mGBA games-folder picker; there is no editor for them. API: `configure_emulator {id, args}`.

## Cartridge and disc systems run by catalogue emulators

Added 2026-10-21. A system with no dedicated runner kind is one file in `catalog/systems/<id>.json` naming a catalogue emulator (`"runner_kind": "emulator"`, `"emulator": "<id>"`). The emulator's own catalogue file says how a game starts: `"kind": "emulator"`, a `launch` argument list where `{rom}` is replaced by the image path as a single argument (no shell), and optional `env`. Scanning such a root imports every supported file as a `rom` media entry with no serial; artwork for these systems is not implemented yet.

| System | Folder scanned | Emulator | Download | Launch arguments |
| --- | --- | --- | --- | --- |
| Dreamcast | `.cdi .gdi .chd` | Flycast v2.7 | GitHub AppImage, published SHA-256 | `-config window:fullscreen=yes {rom}` |
| NES, SNES, Game Boy, Game Boy Color, PC Engine | `.nes .fds .unf`, `.smc .sfc .swc .fig`, `.gb`, `.gbc .gb`, `.pce .sgx` | Mesen 2.1.1 | GitHub `Linux_x64.zip`, published SHA-256; single `Mesen` binary | `--fullscreen {rom}`, run with `LC_ALL=C.UTF-8` |
| Nintendo 64 | `.z64 .n64 .v64` | Rosalie's Mupen GUI v0.9.0 | GitHub AppImage, published SHA-256 | `--fullscreen --quit-after-emulation --nogui {rom}` |

Verified on this host (2026-10-21): all three installed through `marquee emulators install`, 1,487 images from the local `roms/` folders were scanned into the library (Dreamcast 3, NES 245, SNES 42, GB 26, GBC 1,161, PC Engine 6, N64 4), and one game per emulator was launched with the argument lists above. Flycast (`chroma-circuit`) and RMG (`Kumi-Daiko-Beatoff-64`) stayed running until stopped. **Mesen does not work from the agent's shell on this host** (corrected 2026-10-21): the earlier note that it "stayed running" was wrong. With a clean config it quits by itself with exit 0 after a few seconds, with no arguments it exits in under a second with no output, a second launch is killed by a signal, and any command-line parsing such as `--help` or `--testrunner` aborts with a C++ `std::bad_cast`, with or without `LC_ALL=C.UTF-8`. PCSX2 (also a GUI) runs fine from the same shell, so it is Mesen-specific; the cause is unknown. Mesen's Lua test-runner, which would have given headless screenshots, is unusable for the same reason. **Not verified:** the pictures on screen, input, clean exit and playtime for these systems. Because of that Marquee now treats a normal exit within 10 seconds of launch as a failed start ("the emulator closed itself after Ns without a game being played") instead of a played session; the fake test runner is exempt. NES, SNES, Game Boy, Game Boy Color and PC Engine (about 1,480 games) therefore currently have no working emulator.

Not covered yet, and why: **Sega Genesis/Mega Drive** and **Neo Geo Pocket** have no emulator with a checksummed Linux download that Marquee can fetch (ares publishes no Linux build; mGBA publishes no checksums). They need a source-build recipe or a different emulator. **mGBA** is not used for Game Boy because Mesen covers it. The UI maps these systems to platforms and launches them; "Add folder" buttons for them are not built yet (use `marquee library add-root --system <id> <folder>`).

## Firmware requirements

A catalogue entry may list `firmware` files taken from the emulator's own source (via the RetroBIOS profiles): `name`, `aliases`, `required`, an exact `size` when the emulator checks one, the isolated folder it belongs in (`root` = `config` or `data`, plus `dir`), and a note. Marquee reports each file as in place or missing (`emulators list`, and a Firmware section in the emulator dialog), and the BIOS inbox copies any matching file, or one inside a dropped zip, into place without overwriting (a wrong-size file under the right name is refused).

| Emulator | File | Required | Where it goes | Still manual |
| --- | --- | --- | --- | --- |
| Flycast | `dc_boot.bin` (2 MB; alias `dc_bios.bin`) | optional, HLE fallback | `xdg-data/flycast/` | nothing |
| RPCS3 | `PS3UPDAT.PUP` | required | `xdg-config/rpcs3/firmware/` | installing it into RPCS3 (File > Install Firmware or `--installfw`) is not automated |
| xemu | `mcpx_1.0.bin` (512 B), flash ROM (`Complex_4627.bin`, `cerbios.bin`…), `xbox_hdd.qcow2` | required | `xdg-data/xemu/xemu/` | xemu reads the paths from `xemu.toml`; Marquee does not write that file yet |

Verified on this host: the status reports all five as missing (none supplied), and unit tests cover import and the size check. PS1/PS2 BIOS keep their own region-aware import above.

## Package managers: not used

Flatpak, distro packages and loose AppImages found on the host are not discovered or run. Runner rows left by older versions (`flatpak`, `native`, `appimage` sources) are kept only because past sessions reference them; Marquee marks them unusable. The `packages` hints were removed from the catalogue files, the model and the UI.

## Adding another emulator

1. Add `catalog/emulators/<id>.json` (copy `duckstation.json`). Give **either** a GitHub source (`github`, `asset_suffix`, `asset_excludes`, `rolling` when the tag never changes) **or** a direct link:

   ```json
   "download": { "url": "https://example.com/Emu-1.2-linux.tar.gz", "sha256": "<64 hex>", "version": "1.2", "archive_root": "Emu-1.2" }
   ```

   `.AppImage`, `.zip` and `.tar.gz` are supported. A direct link must carry a pinned SHA-256; a GitHub asset must publish one. Marquee verifies it, unpacks into a staging folder and swaps the install in only on success, so an update means changing `version` and `sha256`. Also set `binary` (the executable inside the unpacked archive, relative to `archive_root`), the user-data folder name, and the optional BIOS folder.
2. List the file in `CATALOGUE` in `crates/core/src/emulators.rs`.
3. If it is a new emulator family, add a `RunnerKind` in `model.rs`, map its systems in `runners::preferred_kind`, and add its launch flags to `runners::launch_argv`.

Install, update checks, uninstall, runner registration and isolation then come from the catalogue. A direct link has been installed end to end: Mesen now uses one (pinned 2.1.1 zip and SHA-256), and a clean `--data-dir` install produced a working `Mesen` binary (2026-10-21). A wrong checksum is refused in code but was not tried against a live download. `.7z` and other formats are unsupported; the emulator must honour `XDG_CONFIG_HOME` for its user folder. 

## Game-only windows and headless emulation

Upstream source/docs checked 2026-10-21. Three distinct capabilities:

- **Game-only:** hide the emulator's main/library window; keep the game window, audio and controller input. Marquee's default direction for normal Play.
- **Displayless/headless:** no desktop window; for testing or a separate streaming pipeline. A flag alone does not stream a game into Marquee.
- **Save state:** serialized machine state, unrelated to hiding a window; tied to a game and emulator version.

| Emulator | Upstream finding | Marquee behavior / remaining work |
| --- | --- | --- |
| PCSX2 | [`-nogui`](https://github.com/PCSX2/pcsx2/blob/master/pcsx2-qt/QtHost.cpp) hides the main window and implies batch. | Library argv: `-batch -fullscreen -nogui -- <disc>`. Not displayless; real launch/exit is a live gate. |
| DuckStation | [`-nogui`](https://github.com/stenzek/duckstation/blob/master/src/duckstation-qt/qthost.cpp) hides the main window, exits on shutdown. | Library argv: `-batch -fullscreen -nogui <disc>`. Needs a display and working BIOS. |
| mGBA | Separate [Qt](https://github.com/mgba-emu/mgba/blob/0.10.5/src/platform/qt/main.cpp) and [SDL](https://github.com/mgba-emu/mgba/blob/0.10.5/src/platform/sdl/main.c) frontends; `-f` is fullscreen. | Source pilot builds both. Open selects Qt; `--marquee-sdl -f <game>` selects the game-only frontend. No GB library adapter or displayless mode. |
| RPCS3 | [CLI](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/rpcs3.cpp): `--no-gui`, `--fullscreen`, `--headless`; all need a boot target. | Standalone only. A PS3 adapter should use no-gui plus firmware/install checks. |
| Dolphin | [NoGUI frontend](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/DolphinNoGUI/MainNoGUI.cpp) accepts `x11` or `headless`; may still open a game window. | No recipe or adapter yet. Don't assume a Wayland NoGUI backend. |
| Cemu | [LaunchSettings](https://github.com/cemu-project/Cemu/blob/main/src/config/LaunchSettings.cpp): `-g <game>`, `-f`. | No generic no-GUI option found. Standalone only. |
| PPSSPP | [CLI parser](https://github.com/hrydgard/ppsspp/blob/master/Core/CmdLine.cpp) and [SDL frontend](https://github.com/hrydgard/ppsspp/blob/master/SDL/SDLMain.cpp) handle boot/fullscreen. | Standalone only. Confirm flags against the installed release. |
| xemu | [CLI](https://xemu.app/docs/cli/): `-full-screen`, `-dvd_path <iso>`, `-config_path <config>`. | Standalone only; firmware/HDD/EEPROM readiness needs its own adapter. |
| Flycast, Azahar | Source examined; no verified game-only/headless recipe. | Standalone only. Azahar's room/server binary is not a headless emulator. |

Only the PCSX2 flag change and the existing DuckStation adapter are integrated library behavior; the rest is research, not proof that an installed binary supports the flags.

Next: describe launch presentation as adapter capabilities (`game window`, `main UI`, optional `displayless`), keep a setup/open action even when Play bypasses the main UI, and per adapter validate boot, exit, focus return, audio/controller input, saves, shader prompts and error visibility. Save-state resume and streaming need separate designs.

## Source builds (mGBA pilot)

Implemented 2026-10-21: an opt-in mGBA pilot on Linux x86_64. The other managed emulators keep checksum-verified GitHub binaries. It does not build every emulator or enable new library platforms.

**Use**

1. Needs Git and working **rootless Podman** (a rootful builder is refused; no sudo, no host package installs).
2. **Tools → Emulators → mGBA → Build from source.** The job runs in the backend and survives closing the dialog. Cancel waits for the active subprocess. Failed/cancelled/interrupted jobs retry from fresh staging; a restart reports an interrupted job and does not resume.
3. Pick a specific game folder, then Open. It is mounted read-only; `/` and Home are refused. The wrapper consumes `--marquee-games=<folder>` before invoking mGBA.
4. Qt is the normal Open frontend. Game-only launch uses separate argument fields: `--marquee-sdl`, `-f`, `/absolute/path/to/game.gba` (file mounted individually, no shell expansion). This is not a library session and adds no playtime.
5. Rebuild keeps one previous program and receipt; **Restore previous version** swaps them without touching saves. Remove keeps saves unless "remove with saves" is chosen.

```sh
marquee emulators build mgba
marquee emulators build-status mgba
marquee emulators rollback mgba
```

Browser and Tauri share `build_emulator`, `emulator_build_status`, `cancel_emulator_build` and `rollback_emulator`. Status returns the last 32 KiB of the log. Demo mode never claims to build.

**Recipe and provenance**

- Source: [mGBA 0.10.5](https://github.com/mgba-emu/mgba/tree/26b7884bc25a5933960f3cdcd98bac1ae14d42e2), commit `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`; fetch requests that commit, checkout verifies HEAD, unexpected submodules fail closed.
- [Containerfile](../catalog/source-builds/mgba/Containerfile): Debian base pinned by SHA-256; Qt and SDL built with CMake/Ninja inside rootless Podman. Reviewed recipe, not a generic executor.
- Package repositories are **not snapshot-pinned**; `/opt/mgba/packages.txt` records exact installed versions. Not a signature or bit-for-bit reproducibility claim.
- Both frontends must pass `--version` before promotion; `current/source-build.json` records commit, image ID, recipe/launcher hashes.
- Build subprocesses use argv, null stdin, own process group, per-stage deadlines (up to two hours), cancellation and a 16 MiB log cap. Runtime uses `--network=none --pull=never`.
- Promotion keeps `previous` and restores `current` on receipt-persistence failure; not journaled against power loss.

### Storage and runtime boundary

| Location | Contents |
| --- | --- |
| `<data>/builds/mgba/` | Last status, bounded build log, Podman preflight |
| `<data>/runners/mgba/current/` | Launcher, image identity, receipt, provenance |
| `<data>/runners/mgba/previous/` | One previous program/receipt |
| `<data>/mgba/xdg-config/`, `xdg-data/saves/`, `states/` | Settings, saves, states |
| Rootless Podman image storage | Compiled binary, libraries, layers, package manifest |

The program runs in its local OCI image, so Podman stays a runtime prerequisite; Open detects a missing/pruned image and asks to rebuild. Image layers are a shared Podman cache: removing the Marquee program does not delete them. Abrupt death can leave `.source-*` staging directories and builder cache. Interactive runs expose only the selected game files, managed data directories, the graphics device and the needed display/audio/X11 sockets, never the whole home; `--version` probes expose no mounts, devices or sockets. Controller passthrough, GPU/audio differences, Wayland/X11 rendering and graceful shutdown still need live acceptance. Output is not embedded in Tauri.

**Verification.** On Fedora 44 the pinned recipe compiled both frontends; the CLI completed fetch → build → validate → promote in a separate directory, a second build kept the previous program, and rollback worked. No gameplay or interactive rendering was verified.

**Next gates**

1. Live mGBA game/audio/controller/save/exit acceptance with a user-owned game.
2. GB/GBC/GBA descriptors, import, runner, session tracking and save acceptance; until then it stays standalone.
3. Managed prerequisites, smaller runtime images, storage cleanup, crash-safe promotion.
4. Dolphin is the evaluated next candidate (pin revision and recursive submodules, choose Qt/batch and optional NoGUI); not implemented.
5. Add recipes only where binaries are unavailable or a patch justifies the maintenance.

## Not done

- Emulators other than DuckStation do not launch library games; their systems cannot be scanned yet.
- A live DuckStation Play, quit and duration check has not been run. See [PS1](PS1.md).
- Ports still have no uninstall.
- PCSX2 is not yet on the catalogue.
- Settings profiles ("recommended settings for this machine") are planned, not built; see the [roadmap](ROADMAP.md).
- The new Tools layout has been type-checked and built but not yet looked at in a live window.

## RetroBIOS reference

[Abdess/retrobios](https://github.com/Abdess/retrobios) publishes per-platform BIOS packs and, more usefully, source-verified per-emulator profiles (`emulators/<name>.yml`) listing the files each emulator reads. Findings checked 2026-10-21: PCSX2 accepts any 4-8 MB image with a valid ROM directory and derives its `.rom1/.rom2/.nvm/.mec` names from it; DuckStation identifies images by MD5 across 105 known entries and picks by console region; Flycast has a built-in HLE BIOS and needs real files only for a short list of games and for arcade systems; RPCS3 needs `PS3UPDAT.PUP`; xemu needs four configured files (boot ROM, flash ROM, EEPROM, HDD image). The packs hold copyrighted material (the repository's `NOTICE` says it is not covered by its MIT licence), so Marquee imports files the user supplies and does not fetch them. A per-emulator firmware requirements list built from these profiles is planned work.
