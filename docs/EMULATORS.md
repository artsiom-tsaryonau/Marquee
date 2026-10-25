# Managed emulators and BIOS

Updated 2026-10-24. Marquee installs emulators and holds BIOS images inside its own data folder, so everything can be removed on demand. **Policy (2026-10-21):** Marquee installs, configures and runs emulators itself, from verified archives or reviewed pinned source recipes named in the catalogue. It does not use Flatpak or other package managers, and it no longer discovers host copies of emulators. Setup that an emulator would normally ask the user for is written by Marquee before launch.

## GB/GBC engine selection

Game Boy and Game Boy Color descriptors retain Mesen as their default and list
mGBA as a reviewed alternative. Installing mGBA now registers a library runner;
its catalogue launch template opens Qt with the selected cartridge path and adds
`-f` only for fullscreen. GBA is not enabled by this change.

Settings → Games and controls offers independent GB and GBC defaults. A game’s
installation sidebar offers **Play with**, including **Platform default** to
clear its override. The shared core validates compatibility and availability;
choices persist through restart. The native and browser UI use the same APIs.
Snapshots publish compatible systems and game overrides so displayed readiness
matches launch resolution. An unavailable explicit default/override blocks Play
until repaired or changed, rather than switching engines and save directories.

CLI equivalents:

```sh
marquee runners select runner.mgba.marquee --system gb
marquee runners for-game <edition-id> runner.mesen.marquee
marquee runners for-game <edition-id>  # clear the game override
```

Mesen retains its existing `mesen/xdg-config/Mesen2` data. mGBA writes to its own
`mgba/xdg-data/saves` and `mgba/xdg-data/states` through the managed wrapper.
Selection does not copy, convert or merge battery saves or states. Both engines
retain their existing isolated XDG paths; no personal settings are rewritten.
Tests cover persistence, override precedence, incompatible-engine rejection,
missing-engine blocking and literal game paths. Real GB/GBC video/audio/input,
battery-save persistence, state restoration and streaming remain separate live
gates; the earlier NES/SNES Mesen acceptance does not establish them.

Host check (2026-10-24): the pinned mGBA source recipe completed in an isolated
library and in the main library. A copied DryMouth GB cartridge launched through
the isolated library adapter and exited with code 0 after 87 seconds. This is
process/session evidence; video, audio and keyboard acceptance await owner
confirmation. Main-library installation did not create GB/GBC default settings
or change Mesen’s data. Pilot files and logs remain ignored local artifacts.

## Manager and added collection (planned)

[Marquee manager](MARQUEE_MANAGER.md) will run durable setup/preparation jobs;
Marquee keeps launch profiles and sessions. Existing core installers described
below remain implemented behavior until migrated. The owner has added Switch,
PS3, PS4, original Xbox and Xbox 360 content; [console preparation](CONSOLE_PREPARATION.md)
records the read-only inventory, missing adapters and per-platform gates. Tool
installation/opening is not game installation or compatibility acceptance.

## Current catalogue and integration boundary

Streaming update, 2026-10-24: the [Sonic pilot](STREAMING_GAME_PILOT.md) runs the
managed BlastEm build directly inside Wolf's private Wayland/audio session. The
owner confirmed streamed video/audio/keyboard play and state restoration. Controlled
receiver-loss/reconnect retained the same game/session, followed by normal game
exit. [Owned-viewer tests](STREAMING_VIEWER.md) also confirmed focus/key release,
mouse release and explicit receiver disconnect. Controller, automatic focus
return and the OS window-close button remain open. These
results are separate from local Sonic acceptance and are not implied by the
catalogue matrix below.

Local Mesen update, 2026-10-24: the owner confirmed **Alter Ego (NES)** and
**Uwol: Quest for Money (SNES)** video, audio and keyboard play. NES closed with
exit 0. SNES slot-1 state restored and File → Exit returned 0. An earlier SNES
attempt ended without a numeric exit code; its cause is unresolved. These tests
used fresh isolated profiles and copied inputs. [Exact evidence and remaining
checks](MESEN_ACCEPTANCE.md) distinguish state restoration, native saves and
streamed/controller acceptance.

| Emulator | Platform | Current Marquee capability |
| --- | --- | --- |
| PCSX2 | PS2 | Managed lifecycle, region-aware BIOS setup and disc launch |
| Mesen | NES / SNES / GB / GBC / PCE | Generic cartridge adapters; local NES/SNES video/audio/keyboard and SNES state restore accepted for two titles; GB/GBC/PCE and streaming pending |
| RMG | Nintendo 64 | Generic cartridge adapter; input acceptance pending |
| BlastEm | Genesis / Mega Drive | Pinned source build and direct cartridge launch; no BIOS |
| DuckStation | PS1 | Managed lifecycle plus library-disc launch adapter |
| PPSSPP | PSP | Managed lifecycle and standalone opening |
| RPCS3 | PS3 | Managed lifecycle/standalone opening, [PS3 discovery, reviewed firmware setup and verified ISO registration](PS3.md); direct ISO Library Play implemented; live game acceptance pending |
| Flycast | Dreamcast | Managed lifecycle and generic disc adapter; live game acceptance separate |
| Azahar | Nintendo 3DS | Managed lifecycle and standalone opening |
| Cemu | Wii U | Managed lifecycle and standalone opening |
| xemu | Original Xbox | Managed lifecycle and standalone opening |
| mGBA | GB / GBC / GBA | Opt-in pinned source build; GB/GBC library launch and explicit Mesen/mGBA selection implemented; live GB/GBC acceptance pending. GBA remains standalone. |
| melonDS | Nintendo DS | Pinned source build, `.nds` folder scanning and library Play; saves/states isolated |
| FinalBurn Neo | Neo Geo | Pinned SDL2 source build, intact ZIP scanning, BIOS/legacy-set diagnostics and library adapter; complete matching sets required |
| shadPS4 | PS4 | Managed lifecycle and standalone opening (experimental) |
| KytyPS5 | PS5 | Managed lifecycle and standalone opening (experimental research build) |

PCSX2 is a normal catalogue entry (`catalog/emulators/pcsx2.json`, runner `runner.pcsx2.marquee`): the same verified-download-and-unpack path as every other emulator. The old dedicated installer (`tools.rs`) is gone. An install made by it is adopted as-is on the next refresh (recorded as version `adopted`), and its old `runner.pcsx2.unpacked` row is disabled, so nothing is downloaded twice (checked on this host). The install command `install_pcsx2` remains for the install dialog and delegates to the catalogue. A catalogue entry is implementation of acquisition/opening, not proof of a successful current upstream install, complete configuration isolation, firmware readiness or a playable game. Binary installation requires a matching upstream artifact with a published checksum; source recipes instead verify their pinned commit and record image/recipe receipts. Missing or changed packaging produces an error. Additional platforms still need system descriptors, media import, launch flags, prerequisite checks, save behavior and live acceptance before their games appear as playable library entries. Standalone emulator lifetime is not recorded as game playtime.

## Mesen menu visibility

Library launches pass `--preferences.autoHideMenu=true`, supported by the pinned
Mesen 2.1.1 command-line configuration switches. This applies to existing profiles
as well as new ones. Marquee still controls window versus fullscreen selection;
the operating system's window decorations are separate. Standalone Tools opening
remains available for configuration. Mesen may save this preference in its own
profile; it can be changed in its Preferences dialog.

Reviewed source: [preferences](https://github.com/SourMesen/Mesen2/blob/2.1.1/UI/Config/PreferencesConfig.cs),
[command switches](https://github.com/SourMesen/Mesen2/blob/2.1.1/UI/Utilities/CommandLineHelper.cs),
[menu visibility](https://github.com/SourMesen/Mesen2/blob/2.1.1/UI/ViewModels/MainWindowViewModel.cs).
Earlier gameplay acceptance predates this change; visual auto-hide acceptance
remains separate.

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

"Verified" means: the release was downloaded, matched its published checksum, unpacked, and started with its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_CACHE_HOME`; nothing appeared in `~/.config`, `~/.local/share` or `~/.cache`. It does **not** mean a game was played. This table records the earlier binary-install check. The current integration table above supersedes its adapter coverage; catalogue entries with a configured runner kind can register library runners.

mGBA's published binary remains excluded by the checksum policy; it now has a separate [managed source-build pilot](#source-builds-mgba-pilot). **melonDS** now has a pinned source recipe (and support for its verified ZIP-wrapped AppImage). **Dolphin** needs a different acquisition recipe. **shadPS4** and **KytyPS5** install from verified GitHub release assets (Linux SDL ZIP with an inner AppImage, and a Linux x86_64 tarball respectively); shadPS4 opens the SDL build in Big Picture mode (`-b`), with configured extra arguments preserved. KytyPS5 opens `kyty_launcher`. Neither has a library game adapter yet. Building a program does not add its systems to library import.

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

Verified on this host (2026-10-21): all three installed through `marquee emulators install`, 1,487 images from the local `roms/` folders were scanned into the library (Dreamcast 3, NES 245, SNES 42, GB 26, GBC 1,161, PC Engine 6, N64 4), and one game per emulator was launched with the argument lists above. Flycast (`chroma-circuit`) and RMG (`Kumi-Daiko-Beatoff-64`) stayed running until stopped. **Mesen needed two fixes (2026-10-21).** (1) Its .NET runtime aborts with a C++ `std::bad_cast` on this host (Fedora, ICU libraries) in every code path, even `--help`; `DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1` in the entry's `env` fixes it (checked: the process stays up with it and aborts without it). My earlier note that Mesen \"stayed running\" was wrong. (2) With no `settings.json`, Mesen opens a first-run window (storage location, desktop shortcut and icons, default controller mappings) and, when that closes, starts a second copy of itself and exits. Marquee saw the first process end and the game's arguments were lost. The entry now has a `seed` list that writes `Mesen2/settings.json` (`{"Version":"2.1.1","ConfigUpgrade":1}`) only when missing; Mesen reads that as a first run, creates its own defaults (Xbox pad and arrow keys on every port, checked in the file it wrote) and goes straight to the game. A small official icon is fetched at install (`icon_download`, pinned SHA-256) because the zip contains none. Verified: one Mesen process stayed up 15 s through Marquee from a clean folder. **At that time, not verified:** the pictures on screen, input, clean exit and playtime for these systems. The later [Mesen acceptance](MESEN_ACCEPTANCE.md) supersedes those limits only for its two tested titles. Because of that Marquee now treats a normal exit within 10 seconds of launch as a failed start ("the emulator closed itself after Ns without a game being played") instead of a played session; the fake test runner is exempt.

The earlier Genesis acquisition gap is now covered by the pinned BlastEm source recipe and library adapter described below. Neo Geo Pocket still needs its own managed acquisition recipe and library adapter. mGBA remains a standalone source-built tool; Mesen supplies the current Game Boy library adapter.

## Installing what you need

Nothing about an emulator is committed except its small recipe in `catalog/emulators/<id>.json`. Programs, settings, saves and BIOS live in Marquee's data folder, and `.gitignore` covers anything an install or run could leave in the repo. Three ways to get them, all verified downloads into that folder:

- **UI:** Tools has an "Install what my library needs (N)" button (shown only when something is missing; the tooltip lists which), and each emulator card has its own install.
- **CLI:** `marquee emulators install-needed` installs every integrated emulator that covers a system you have a library folder for and skips source builds; `marquee emulators install <id>` does one.
- **Script:** `scripts/install-emulators.sh` runs the same thing (or the ids you give it) from a clone. Safe to re-run.

Checked: in a clean data folder, with only the NES and Dreamcast folders registered, `install-needed` installed Flycast and Mesen, and a second run did nothing.

## Keyboard controls

A game is only playable if the emulator has input bindings, and a seeded config can skip the step where an emulator writes its defaults. Found 2026-10-21: the PCSX2 file Marquee seeded had no `[Pad1]` section, so no key reached PS2 games. Marquee now adds PCSX2's own default keyboard layout for controller 1 (the one its auto-mapper writes: arrows for the D-pad, I/J/K/L for the face buttons, Q/E and 1-4 for the shoulders and sticks' buttons, W/A/S/D for the left stick, T/F/G/H for the right, Backspace for Select, Return for Start) only when `[Pad1]` is missing, so a layout you change is kept. Checked on this host: the section was written, PCSX2 launched and stayed running, and the bindings were still in the file after it exited; the keys themselves were not pressed. Status of the others: DuckStation and Mesen write complete keyboard defaults themselves (checked in their files); Flycast has a built-in keyboard map and its seeded file only holds window settings (not tested with keys); **RMG (N64) has no input profile in its config and is probably affected too**: its input plugin reads per-controller profile sections with SDL scancodes, which Marquee does not write yet.

## Window or fullscreen

Games open in a **window** by default. Settings → Games → "Open games in" (setting `display_mode`, API `set_display_mode`) switches every emulator to fullscreen, effective on the next Play. PCSX2 and DuckStation get `-nofullscreen` or `-fullscreen`. Catalogue emulators take `fullscreen_args` or `windowed_args` from their entry: Mesen and RMG add `--fullscreen` only in fullscreen mode, Flycast gets `-config window:fullscreen=yes` or `=no` because it remembers the last mode. Checked on this host: the command lines switch with the setting and PCSX2 and Flycast stayed running in both modes; I could not see the windows, so the actual window size and decorations are unverified. A window or fullscreen change you make inside an emulator is still that emulator's own to remember (Mesen and RMG), so a launch does not reset it.

## Keeping the library to real games

`marquee prune` (API `prune_library`) removes library entries whose files no longer exist, but only ones never played, not favourites and without a custom title; anything with history stays and shows as missing. Marquee never deletes game files itself.

On 2026-10-21 the local homebrew folders were cleaned by moving 730 files that are not playable games (demos, test ROMs, music and sound editors, tools and utilities, multi-ROM menus, picture/movie files, e-books, adult material, readme/txt files) into `roms/_removed/<system>/`, with `roms/_removed/MANIFEST.txt` recording each original path and the reason. Moving a file back and rescanning restores it. The classification was by filename, so a few odd entries remain in either direction. The library went from 1,487 to 780 entries (Game Boy Color 1,161 to 457, NES 245 to 239, SNES 42 to 38, Game Boy 26 to 22). Duplicate versions of the same game (for example JetPac V0.01 to V0.04) were kept.

## Firmware requirements

Catalogue entries can also carry `seed` (config files written before launch when missing, to skip first-run wizards, relative to the emulator's config folder) and `icon_download` (an icon with a pinned SHA-256, for archives that contain none).

A catalogue entry may list `firmware` files taken from the emulator's own source (via the RetroBIOS profiles): `name`, `aliases`, `required`, an exact `size` when the emulator checks one, the isolated folder it belongs in (`root` = `config` or `data`, plus `dir`), and a note. Marquee reports each file as in place or missing (`emulators list`, and a Firmware section in the emulator dialog), and the BIOS inbox copies any matching file, or one inside a dropped zip, into place without overwriting (a wrong-size file under the right name is refused).

| Emulator | File | Required | Where it goes | Still manual |
| --- | --- | --- | --- | --- |
| Flycast | `dc_boot.bin` (2 MB; alias `dc_bios.bin`) | optional, HLE fallback | `xdg-data/flycast/` | nothing |
| RPCS3 | `PS3UPDAT.PUP` | required | installed output: `xdg-config/rpcs3/dev_flash/` | reviewed manager setup in Configure; storing a PUP alone is not readiness |
| xemu | `mcpx_1.0.bin` (512 B), flash ROM (`Complex_4627.bin`, `cerbios.bin`…), `xbox_hdd.qcow2` | required | `xdg-data/xemu/xemu/` | xemu reads the paths from `xemu.toml`; Marquee does not write that file yet |

The generic inbox imports have unit coverage for size checks. RPCS3 uses its separate reviewed setup and installed-output checks; its PUP inbox is no longer presented as installation. PS1/PS2 BIOS keep their own region-aware import above.

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
| mGBA | Separate [Qt](https://github.com/mgba-emu/mgba/blob/0.10.5/src/platform/qt/main.cpp) and [SDL](https://github.com/mgba-emu/mgba/blob/0.10.5/src/platform/sdl/main.c) frontends; `-f` is fullscreen. | Source pilot builds both. Open selects Qt; `--marquee-sdl -f <game>` selects the game-only frontend. GB/GBC library Play uses Qt with the game path; no displayless mode. |
| RPCS3 | [CLI](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/rpcs3.cpp): `--no-gui`, `--fullscreen`, `--headless`; all need a boot target. | Library Play uses no-gui with verified ISO identity and installed-firmware checks; configuration remains separate. |
| Dolphin | [NoGUI frontend](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/DolphinNoGUI/MainNoGUI.cpp) accepts `x11` or `headless`; may still open a game window. | No recipe or adapter yet. Don't assume a Wayland NoGUI backend. |
| Cemu | [LaunchSettings](https://github.com/cemu-project/Cemu/blob/main/src/config/LaunchSettings.cpp): `-g <game>`, `-f`. | No generic no-GUI option found. Standalone only. |
| PPSSPP | [CLI parser](https://github.com/hrydgard/ppsspp/blob/master/Core/CmdLine.cpp) and [SDL frontend](https://github.com/hrydgard/ppsspp/blob/master/SDL/SDLMain.cpp) handle boot/fullscreen. | Standalone only. Confirm flags against the installed release. |
| xemu | [CLI](https://xemu.app/docs/cli/): `-full-screen`, `-dvd_path <iso>`, `-config_path <config>`. | Standalone only; firmware/HDD/EEPROM readiness needs its own adapter. |
| Flycast, Azahar | Source examined; no verified game-only/headless recipe. | Standalone only. Azahar's room/server binary is not a headless emulator. |

Only the PCSX2 flag change and the existing DuckStation adapter are integrated library behavior; the rest is research, not proof that an installed binary supports the flags.

Next: describe launch presentation as adapter capabilities (`game window`, `main UI`, optional `displayless`), keep a setup/open action even when Play bypasses the main UI, and per adapter validate boot, exit, focus return, audio/controller input, saves, shader prompts and error visibility. Save-state resume and streaming need separate designs.

## Source builds (mGBA pilot)

Implemented 2026-10-21: reviewed Linux x86_64 source recipes for mGBA, melonDS and FinalBurn Neo. The other managed emulators keep checksum-verified GitHub binaries. Source support does not imply that every system handled upstream has a Marquee library adapter.

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
2. GB/GBC runner selection and session integration are implemented; real game/save acceptance remains open. GBA descriptor/import remains unimplemented.
3. Managed prerequisites, smaller runtime images, storage cleanup, crash-safe promotion.
4. Dolphin is the evaluated next candidate (pin revision and recursive submodules, choose Qt/batch and optional NoGUI); not implemented.
5. Add recipes only where binaries are unavailable or a patch justifies the maintenance.

## Historical gaps (2026-10-21; current matrix above supersedes this list)

- At that date, emulators other than DuckStation lacked library launch adapters.
- A live DuckStation Play, quit and duration check has not been run. See [PS1](PS1.md).
- Ports still have no uninstall.
- At that date, PCSX2 was not yet on the catalogue; it is now integrated.
- [DuckStation settings profile v1](SETTINGS_PROFILES.md) is implemented with conservative host detection, custom options and per-key user-edit preservation. Profiles for other emulators and live presentation acceptance remain open.
- The new Tools layout has been type-checked and built but not yet looked at in a live window.

## RetroBIOS reference

[Abdess/retrobios](https://github.com/Abdess/retrobios) publishes per-platform BIOS packs and, more usefully, source-verified per-emulator profiles (`emulators/<name>.yml`) listing the files each emulator reads. Findings checked 2026-10-21: PCSX2 accepts any 4-8 MB image with a valid ROM directory and derives its `.rom1/.rom2/.nvm/.mec` names from it; DuckStation identifies images by MD5 across 105 known entries and picks by console region; Flycast has a built-in HLE BIOS and needs real files only for a short list of games and for arcade systems; RPCS3 needs `PS3UPDAT.PUP`; xemu needs four configured files (boot ROM, flash ROM, EEPROM, HDD image). The packs hold copyrighted material (the repository's `NOTICE` says it is not covered by its MIT licence), so Marquee imports files the user supplies and does not fetch them. A per-emulator firmware requirements list built from these profiles is planned work.


## Nintendo DS and Neo Geo library adapters

Use **Games → Add Nintendo DS folder** or **Add Neo Geo folder**. DS archives
must first be extracted to `.nds` files. Neo Geo arcade sets stay zipped, with
`neogeo.zip` and required parent sets beside the selected game. The BIOS ZIP is
excluded from playable entries. Readiness is rechecked when taking a snapshot;
restored dependencies do not require deleting and re-importing an installation.
A present BIOS file is not proof of checksum/driver compatibility.

```sh
marquee library add-root --system ds /path/to/ds
marquee library add-root --system neogeo /path/to/neogeo
marquee scan
marquee emulators build melonds
marquee emulators build fbneo
```

Both recipes use the existing cancellable rootless Podman builder, keep a
previous program for rollback and record commit, image, recipe and launcher
hashes. No host development packages are installed. The source-build Update
button rebuilds the pinned recipe. A failed build retains the working program.

- melonDS 1.1: `b86390e4428bf38ce4c1ce0e9ca446d6d25955e8`, from
  <https://github.com/melonDS-emu/melonDS>. Builds the Qt 6/SDL frontend. The
  first-run config selects DS direct boot with built-in BIOS replacements;
  DSi and external firmware boot are outside this adapter. Arrow keys move,
  X/Z are A/B, S/A are X/Y, Q/W are L/R, Return starts, Backspace selects;
  mouse controls the lower touch screen. Source-launcher saves/states use
  the emulator's `xdg-data/saves` and `xdg-data/states`, with selected cartridges
  mounted read-only. Existing config is preserved. Validation uses offscreen
  `--help`, separately from game boot acceptance.
- FBNeo SDL2: `15272edc8824997b946f31fbe2306462b7a6624f`, from
  <https://github.com/finalburnneo/FBNeo>. The recipe records a portability patch
  replacing two Windows `UINT` menu fields with equivalent `unsigned int`.
  The upstream Linux release ZIP inspected on this host contains documentation
  but no executable. The launcher translates a selected ZIP path into a driver
  name, mounts its sibling ROM directory read-only and preserves data/settings.
  The build probe is no-ROM usage, which exits before initializing SDL.

The supplied `samsh5nd.zip` is an old split set: graphics and audio-CPU data,
without main-program or sound-sample ROMs. That name is not a driver in the
pinned FBNeo revision. Marquee keeps the archive visible with an explanation
and prevents launch; it does not rename it into a different set. Supply a
complete ROM set matching the selected FBNeo revision plus its BIOS/dependencies.
Full DAT/CRC-based validation and guided ROM-set repair remain follow-up work.
Prune keeps blocked entries while their media files still exist.


Live checks on this host (2026-10-21): both pinned source recipes compiled and
passed their displayless probes. NFS Underground's `SLUS_208.11` ELF executed
in PCSX2 and a disposable Marquee session ended cleanly after 18 seconds.
Pokémon Edición Platino reached its Game Freak intro in source-built melonDS,
with an inspected game-window capture and a clean 20-second Marquee session
using the X11 backend. These checks do not establish audio/controller quality
or an in-game save/load round trip. Test history/settings stayed in disposable
libraries. The actual three game files were registered in the user's library.

Source wrappers preserve the host XDG environment for Podman image lookup,
then pass isolated emulator XDG directories into the container. They use an
init process and a private user-owned runtime directory with selected display
and audio sockets. Managed wrapper changes can be applied without recompiling
the pinned emulator; receipts are updated, interrupted receipt updates recover,
and locally edited wrappers are left alone. Save paths in melonDS's first-run
config are host paths, also mounted at the same locations in the container, so
they remain meaningful if the previous verified binary is restored.


## Genesis / Mega Drive

The `genesis` adapter accepts extracted `.md`, `.gen`, `.smd` and `.bin`
cartridges. Archive intake recognises `.md`/`.gen`/`.smd` inside ZIP or 7z,
skips accompanying text and registers a Genesis library root. Source archives
are kept by default. A bare ambiguous `.bin` archive is not guessed as Genesis.

```sh
marquee intake "Streets of Rage 2 (USA).zip"
marquee emulators build blastem
```

BlastEm's standalone SDL2 frontend is built from the
[libretro upstream-tracking repository](https://github.com/libretro/blastem),
pinned to `ab743653ed110eeb7b1fe64d00cc5f9279c2bfec`. This uses the normal
managed rootless Podman builder, version/help probes and installation receipts.
No host development packages are installed. Genesis cartridges need no BIOS;
Sega CD and 32X are outside this adapter. Library Play passes the selected
cartridge path literally, mounted read-only; fullscreen uses upstream `-f`.
Arrows move, A/S/D are A/B/C, Enter is Start, and Esc opens BlastEm's menu.

Upstream Linux uses HOME rather than XDG for its files. The launcher maps
`$HOME/.config` and `$HOME/.local/share` onto Marquee's BlastEm `xdg-config`
and `xdg-data` folders. Settings live in `xdg-config/blastem/blastem.cfg`;
saves/states use `xdg-data/blastem/<ROMNAME>/`. Existing settings are preserved.

The managed Genesis wrapper selects BlastEm's `-g` SDL software renderer as
its portable baseline, avoiding host/container OpenGL driver and swap-interval
mismatches. No graphics device is exposed for this adapter. Source wrappers
pre-create the private PulseAudio socket directory as the mapped user with
mode 0700; otherwise Podman creates a namespace-root parent that libpulse
rejects, causing SDL to fall back to an unavailable ALSA device.

The pinned revision parses `-g` without applying it. The Containerfile records
a one-line source patch applying `video.gl = off` in memory before rendering
initializes when `-g` is present. It leaves persistent user settings untouched.
The patched recipe has the version label `standalone-sdl`.

Live acceptance on 2026-10-21: the patched production recipe launched the
supplied Streets of Rage 2 (USA) cartridge through a fresh disposable Marquee
library. The inspected window reached the opening story sequence, PulseAudio
reported an uncorked float32 stereo 48 kHz stream, and closing the game window
ended the 20-second session with exit 0. Neither the SDL audio failure nor the
OpenGL vsync dialog appeared. That earlier run did not verify audio quality, input or save/load. Test playtime
stayed out of the personal library.


### Sonic 2 live acceptance (2026-10-22)

The existing Sonic The Hedgehog 2 (World, Rev B / Sonic Compilation) ZIP was
prepared through the manager in a fresh isolated library, using the installed
pinned BlastEm image/commit above. No new game download or historical-browser
job adoption occurred. Source SHA-256 remained
`d8da4ea73dbea911886f1fc40b479e2363bbf9b2ed92d77e0557e853cd215735`.

The owner confirmed visible gameplay, keyboard movement/play (arrows and W),
audible game sound, successful quick-state restore and normal window close.
Backtick created a 141,640-byte `quicksave.state`; L restored the earlier game
position. Marquee recorded one successful 363-second session with exit 0 and one
play. The failed pre-fix launches did not count as plays. The personal library
remained at 856 editions / 29 sessions; test settings, state and playtime stayed
under ignored `data/manager-pilot/sonic-live-20261022/`.

Two launch issues were exposed:

- Relative `--data-dir` values reached runner XDG/container mount arguments as
  relative paths and failed with exit 125. Core paths now resolve to canonical
  absolute directories before constructing runners or taking the library lease.
- SDL's X11 software path failed with `MIT-SHM / X_ShmPutImage BadValue` on this
  container/display combination. The managed BlastEm wrapper now prefers native
  Wayland when its active socket is available and forwarded. Explicit
  `SDL_VIDEODRIVER` values are respected. IPC remains private; no host-IPC or GPU
  access was added. X11-only live acceptance remains open. SDL's driver override
  is documented in the [official SDL2 reference](https://wiki.libsdl.org/SDL2/SDL_HINT_VIDEODRIVER).

The wrapper retains stdout/stderr from the latest attempt in its private
`xdg-cache/blastem-last-launch.log` (mode 600, replaced on next launch). It uses
an ordinary file so closing Marquee cannot break the game's output pipe. This
is a last-attempt diagnostic, not a size-limited historical log service. The
accepted Wayland run emitted nonfatal libdecor/EGL probe warnings; those alone
did not indicate failed gameplay.

This closes a title-specific keyboard/audio/emulator-save-state/clean-exit gate.
Physical controller input, battery-backed in-game saving, persistence across a
second emulator launch, and focus return to the native Marquee window remain
untested. The launch was through the real core/CLI, not a fake runner. A timed
8-second diagnostic probe was explicitly excluded from Enter/crash conclusions.
New browser acquisition through real provider gates and manager torrent adoption
remain separate milestones.


### Quiver native-port pilot

Tools also offers the 2 Ship 2 Harkinian 5.0.1 Linux engine through an exact
release/archive/executable pin. Approved local app-list entries can open that
recipe for review. Installation does not include Majora’s Mask game data or
prove gameplay. The source alias, integrity tests and remaining manager/setup
gates are recorded in [Quiver Q2a](QUIVER_INTEGRATION.md#q2a--pinned-2-ship-engine-recipe-2026-10-25).

## Ocarina native-port pilot

Ship of Harkinian is a native port, not an N64 emulator. Its managed recipe pins
9.3.1 and runs with an installation-local `SHIP_HOME` plus the AppImage
extract-and-run fallback for Fedora hosts without FUSE 2. The supplied PAL 1.1
input passed the pinned supported-hash check, generated its assets and reached
file select in an isolated Marquee launch. Gameplay and persistent native saves
remain acceptance gates. See [Quiver Q2c](QUIVER_INTEGRATION.md#q2c--ocarina--shipwright-pilot-2026-10-25).
