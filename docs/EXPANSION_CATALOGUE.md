# Marquee expansion catalogue

Researched 2026-10-20. These are integration candidates and product proposals, not implemented adapters or locally tested compatibility claims. Follow the linked upstream documentation when implementing each adapter; releases, supported game revisions and Linux packaging change.

**As of 2026-10-20:** a PS2/PCSX2 offline import-to-play slice is in the tree (ISO scan, CLI, Tauri commands). It is not a finished adapter: live launch/exit is unverified, and nothing else in this catalogue is implemented. Demo UI still runs in the browser with sample data. See [PS2.md](PS2.md).

## Product direction

Make Marquee a personal gaming console with a library spanning original console releases, native PC ports, modern store versions and remote machines. A game page can offer multiple ways to play while keeping each installation, save location, achievement set and compatibility report separate.

For example, an Ocarina of Time page could group an N64 dump, Ship of Harkinian and a separate randomizer profile. A Jak page could group PS2 emulation and an OpenGOAL installation. A PS5 shelf could contain a console Remote Play shortcut and explicitly experimental local entries.

Suggested navigation: Home, Library, Systems, Collections, Ports & Engines, Downloads, Tools. Each system gets cover art, controller diagrams, installed runner status, firmware requirements, game count and collections. Avoid empty shelves unless the user follows that system.

Priority labels below are Marquee implementation recommendations, not upstream compatibility ratings:

- **First:** a good candidate for the first real integrations.
- **Next:** useful once discovery, launching and saves work reliably.
- **Experimental:** opt-in; validate a specific build, game and machine before promising playability.

## PlayStation family

| System | Candidate | Marquee experience | Priority and requirements |
| --- | --- | --- | --- |
| PS1 | [DuckStation](https://github.com/stenzek/duckstation) | Disc-library scanning, multi-disc grouping, memory-card shortcuts, per-game launch options | First. Configure the BIOS and supported disc formats through the emulator. |
| PS2 | [PCSX2](https://pcsx2.net/docs/setup/running/) | A PS2 shelf, per-game renderer/settings profiles, memory-card management, compatibility links | **First — slice started 2026-10-20** (ISO import, argv, sessions). Per-game PCSX2 settings, memory cards and compatibility links are not in that slice. [PCSX2 requires a BIOS](https://pcsx2.net/docs/setup/bios/); show missing prerequisites before Play. Live launch pending: [PS2.md](PS2.md). |
| PSP | [PPSSPP](https://www.ppsspp.org/) | A portable-games shelf, upscale presets, controller mappings | First. Keep PSP separate from PS1 games distributed for PSP. |
| PS3 | [RPCS3](https://github.com/RPCS3/rpcs3) | Import installed game folders and IDs; show firmware/setup state, version-specific notes and a first-launch shader-compilation explanation | Next. Follow upstream setup and per-title compatibility; avoid universal performance promises. |
| PS Vita | [Vita3K](https://vita3k.org/) | Import installed titles; expose emulator setup and per-game compatibility | Experimental. Offer only after testing the chosen release and title. |
| PS4 | [shadPS4](https://github.com/shadps4-emu/shadPS4) | Discover installed games, retain CUSA IDs, use an explicit experimental badge, link the upstream compatibility report | Experimental. Upstream calls it early-stage. The emulator core and Qt launcher are separate; the adapter must distinguish them. |
| PS5 local execution | [KytyPS5](https://github.com/KytyPS5/KytyPS5) | An optional research runner with build pinning and per-game test records | Experimental. The current upstream describes a PS5 translation layer for Windows/Linux and reports booting 2D/3D games. That establishes a project to evaluate, not broad library playability on the user's Fedora machine. |
| PS4 / PS5 console streaming | [chiaki-ng](https://github.com/chiaki-ng/chiaki-ng) | A console tile that opens a configured Remote Play session | Next. Requires the user's real console and pairing. Treat the console session as the launch target; do not promise direct per-game boot unless the client exposes it. |

PS5 should therefore have two separate runner choices: **Remote Play** and **Experimental local**. A game being listed in a library is distinct from a verified way to run it. Preserve the original [Kyty](https://github.com/InoriRus/Kyty) as historical context rather than confusing it with the separate KytyPS5 project.

The RPCS3 website blocked automated access during this research; its own GitHub repository confirmed the Linux emulator and linked its setup documentation. No live compatibility percentages were inferred. No emulator was installed or benchmarked during this work.

## Other systems worth supporting

| Systems | Candidate runners | Integration idea | Order |
| --- | --- | --- | --- |
| NES, SNES, Master System, Game Gear, PC Engine and other supported classics | [Mesen](https://github.com/SourMesen/Mesen2), [ares](https://github.com/ares-emulator/ares), selected [RetroArch](https://github.com/libretro/RetroArch) cores | Shared classic-console shelf with configurable per-system defaults | First; select and test an actual core per system. |
| Game Boy / Color / Advance | [mGBA](https://mgba.io/) | Handheld shelf, aspect-ratio presets, matching save-file shortcuts | First. |
| Nintendo 64 | [Mupen64Plus](https://github.com/mupen64plus/mupen64plus-core), ares or a vetted libretro core | Original-console option alongside native ports; per-title input profiles | First. |
| GameCube / Wii | [Dolphin](https://dolphin-emu.org/) | GameCube and Wii shelves, separate controller types, per-game Wii input profiles | First. Wii games can need different input devices and mappings. |
| Nintendo DS / DSi | [melonDS](https://github.com/melonDS-emu/melonDS) | Dual-screen layout presets, touch-input guidance, handheld shelf | Next. Record firmware and content prerequisites from the chosen build. |
| Nintendo 3DS | [Azahar](https://github.com/azahar-emu/azahar) | Per-game screen layouts and installed-title import | Next. Verify the current setup and supported content formats. |
| Wii U | [Cemu](https://cemu.info/) | Installed title/update/DLC grouping and gamepad-display guidance | Next. Keep updates/DLC subordinate to the base title. |
| Nintendo Switch 1 | [Eden](https://git.eden-emu.dev/eden-emu/eden) | Optional adapter for an existing user installation; explicit setup and per-game test status | Next after validating the current release and official distribution path. Upstream lists Linux builds. No Switch 2 support is inferred. |
| Sega Saturn | [Mednafen](https://mednafen.github.io/documentation/ss.html), [Ymir](https://github.com/ymir-emu/Ymir) | Saturn library, disc grouping, runner alternatives for individual games | Next. |
| Dreamcast / NAOMI / Atomiswave | [Flycast](https://github.com/flyinghead/flycast) | Dreamcast shelf and arcade sub-collections with their own BIOS/content checks | Next. |
| Original Xbox | [xemu](https://github.com/xemu-project/xemu) | Discover game images, expose system-file setup, track individual compatibility | Next. Do not assume arbitrary Xbox disc images are directly launchable. |
| Xbox 360 | [Xenia Canary](https://github.com/xenia-canary/xenia-canary) | Title-ID-based imports, selected patches, build pinning | Experimental for Marquee. Upstream lists Linux builds; its [quickstart](https://github.com/xenia-canary/xenia-canary/wiki/Quickstart) also discusses Proton. Test native and Proton profiles independently. |
| Xbox One / Series | Custom runner or remote-session placeholder | Show owned/remote entries without claiming verified local execution | Research only. [XWine1's organization](https://github.com/xwine1) is a project to monitor; this research did not establish a supported Fedora launch path. |
| Arcade machines | [MAME](https://www.mamedev.org/) | Vertical shooters, fighters, beat-'em-ups, steering-wheel and light-gun collections | Next. ROM-set versions and dependencies belong to the selected runner profile. |
| DOS | [DOSBox Staging](https://www.dosbox-staging.org/) | Import an existing installation and keep a separate configuration per game | Next. |
| Classic adventures and supported RPGs | [ScummVM](https://www.scummvm.org/compatibility/) | Import existing game registrations; launch by game ID, with manuals and subtitles/language information | Next. Match compatibility to the installed stable release; the linked default chart describes development builds. |

RetroArch is a frontend for cores, not one universal emulator. Store the core identity and version with each launch profile. Prefer a standalone runner when its setup, game-specific configuration or compatibility reporting makes the experience clearer.

## Native ports, decomps, recompilations and replacement engines

These categories should have separate catalogue tags. A source decompilation repository does not automatically provide an end-user port. A recipe should specify an actual runnable release, supported game-data revision, installation steps and update policy.

### Console-derived ports

| Game / family | Project | Marquee recipe |
| --- | --- | --- |
| Ocarina of Time | [Ship of Harkinian / Shipwright](https://github.com/HarbourMasters/Shipwright) | Check a supported ROM revision, prepare the project's asset archive, launch a native release. |
| Majora's Mask | [2Ship2Harkinian](https://github.com/2ship2harkinian/2ship2harkinian) | A separate port recipe with its own supported inputs and save directory. |
| Majora's Mask | [Zelda64Recomp](https://github.com/Zelda64Recomp/Zelda64Recomp) | Supported ROM import, native release installation, separate mod profiles. Upstream still describes Ocarina support as forthcoming; do not expose it as an available release based on the project name. |
| Super Mario 64 | [sm64ex](https://github.com/sm64pc/sm64ex) | Supported-ROM build recipe; report a source build as such rather than promising a ready binary. |
| Super Mario 64 co-op | [sm64coopdx](https://github.com/coop-deluxe/sm64coopdx) | A distinct installation/profile with its own mods and multiplayer settings. |
| Mario Kart 64 | [SpaghettiKart](https://github.com/HarbourMasters/SpaghettiKart) | A strong couch-racing candidate. Upstream documents a specific US ROM and Linux AppImage asset preparation. |
| Star Fox 64 | [Starship](https://github.com/HarbourMasters/Starship) | Native-port recipe, supported dump validation and controller profile. |
| Jak and Daxter / Jak II / Jak 3 | [OpenGOAL](https://opengoal.dev/docs/usage/installation/) | One engine/toolchain, separate game installations. The [FAQ](https://opengoal.dev/docs/faq/) lists the original PS2 retail games; console re-releases are not interchangeable inputs. Check readiness separately for each game's current release. |
| Sonic Unleashed | [UnleashedRecomp](https://github.com/hedge-dev/UnleashedRecomp) | A recipe for the Xbox 360 version's required files; keep it separate from PS3/Wii/PS2 versions of the title. |
| Sonic Mania | [Sonic Mania Decompilation](https://github.com/RSDKModding/Sonic-Mania-Decompilation) and [RSDKv5](https://github.com/RSDKModding/RSDKv5-Decompilation) | Match engine/game logic and the user's supported data files. |
| Sonic 1 / Sonic 2, 2013 versions | [RSDKv4](https://github.com/RSDKModding/RSDKv4-Decompilation) | Validate the supported 2013 game data; a Mega Drive cartridge dump is a different input. |
| Sonic CD, 2011 version | [RSDKv3](https://github.com/RSDKModding/RSDKv3-Decompilation) | Validate that release's game data; keep the original Sega CD emulation profile separate. |
| Dinosaur Planet prototype | [Dinosaur Planet: Recompiled](https://github.com/DinosaurPlanetRecomp/dino-recomp) | Optional preservation/research entry. Do not offer the prototype data for download; validate availability and prerequisites before making an installable recipe. |

### PC engines and source ports

| Game / family | Project | What the recipe manages |
| --- | --- | --- |
| Morrowind | [OpenMW](https://github.com/OpenMW/openmw) | Existing game-data selection, configuration, mod order and separate profiles. |
| Fallout 1 / 2 | [Fallout CE](https://github.com/alexbatalov/fallout1-ce), [Fallout 2 CE](https://github.com/alexbatalov/fallout2-ce) | Identify existing game files and install the selected replacement executable. |
| Diablo / Hellfire | [DevilutionX](https://github.com/diasurgical/DevilutionX) | Validate the required data archives; keep content availability distinct from engine installation. |
| Doom family and compatible mods | [UZDoom](https://github.com/UZDoom/UZDoom) | IWAD selection, ordered mod files, separate saves and launch profiles. |
| Doom 3 | [dhewm3](https://github.com/dhewm/dhewm3) | Base-game and expansion data detection, version-specific configuration. |
| Half-Life-compatible content | [Xash3D FWGS](https://github.com/FWGS/xash3d-fwgs) | Engine, compatible game libraries and data matching; not every mod is interchangeable. |
| Tomb Raider I / II / III | [TRX](https://github.com/LostArtefacts/TRX) | Separate per-game data import and native engine configuration. |
| RollerCoaster Tycoon 2 | [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2) | Game-data discovery, scenarios and optional RCT1 content when supported. |
| Theme Hospital | [CorsixTH](https://github.com/CorsixTH/CorsixTH) | Original data selection and a native launch profile. |
| Heroes of Might and Magic III | [VCMI](https://github.com/vcmi/vcmi) | Supported game data, expansions and mod profiles. |
| Infinity Engine games | [GemRB](https://github.com/gemrb/gemrb) | A separate game-specific recipe and compatibility check for each supported title. |
| Classic Westwood-style RTS | [OpenRA](https://github.com/OpenRA/OpenRA) | Separate game/mod profiles; follow upstream content acquisition rules. |
| Transport Tycoon-style simulation | [OpenTTD](https://www.openttd.org/) | A useful standalone game integration with its own content/settings management. |
| Cave Story | [NXEngine-evo](https://github.com/nxengine/nxengine-evo) | Correct game-data package and native runner; preserve the selected language/version. |

For the first catalogue, prioritize Shipwright, Zelda64Recomp, OpenGOAL, SpaghettiKart, Starship, sm64coopdx, OpenMW, DevilutionX, Fallout CE and UZDoom. This is a product recommendation: it covers console nostalgia, native ports, cooperative play, RPGs and modded shooters without requiring every adapter at once.

## ROM and disc-library handling

Treat “ROM” as a library source, not a file extension. The importer should understand the media structure, then ask the chosen runner whether it supports it.

- **Cartridges:** system-specific extensions and content hashes. Preserve region and revision. Normalize N64 byte order for identification without silently replacing the user's original file.
- **Disc images:** supported ISO, CUE/BIN, CHD and other formats belong in individual system/runner definitions. Never advertise CHD support globally. Index a CUE as a game rather than treating every referenced track as another title.
- **Multi-disc releases:** group disc files into a single library edition with an explicit disc chooser or playlist, if the selected runner supports it.
- **Installed console games:** parse the runner's installation registry or title metadata. Do not list updates, DLC, firmware installers and game executables as unrelated games.
- **Arcade sets:** record runner version, set identity, parent/clone dependencies and required disk assets. A ZIP is not enough information to declare an arcade game ready.
- **Patches and translations:** record the base dump hash, patch version and resulting variant. Keep the original intact; never apply a patch to a guessed revision.
- **Duplicates:** identify identical content, but retain intentional differences in region, language, revision, mods and save locations. Let users group editions without deleting files.
- **Large libraries:** background indexing, incremental rescans, visible progress, cancel/resume, on-demand hashing and a virtualized grid. Do not hash a whole NAS before showing its first games.
- **Removable disks / NAS:** retain entries when a volume is offline; show “Connect library drive” rather than “Uninstalled.”
- **Secrets and firmware:** represent what is missing, open upstream setup instructions, and let the user supply the required files. Marquee's own catalogue should distribute only content it has permission to distribute.

A useful import flow is: choose folder → detect systems → review matches and unresolved files → select existing runners → resolve required setup → add to library. A folder name alone should not determine the game or its readiness.

## Collections that make Marquee distinctive

These are proposed Marquee features. They can be implemented as saved queries plus optional manual curation, with a visible reason each game appears.

| Collection | How it works |
| --- | --- |
| PlayStation generations | PS1 → PS2 → PS3 → PS4 → PS5 shelves, PSP and Vita alongside; filter to entries with a runnable profile. |
| Same game, new life | Group original emulation, decomp/recomp, PC release and remaster. Let the user compare visuals, input, saves and setup requirements before choosing a default. |
| Native revival | Only installed native ports; optionally suggest a port when a compatible user-supplied game is detected. |
| One ROM, several adventures | Original, supported randomizer seeds, translation and overhaul profiles beneath one base game. Preserve independent saves and exact input hashes. |
| Couch night | Games with local multiplayer metadata, enough connected controllers and compatible input profiles. Show two-, three- and four-player choices. |
| Arcade evening | A playlist of short arcade sessions, with a rotating “next game” recommendation after exit. No fake cross-emulator save suspension. |
| Finish this weekend | Unfinished games under a configurable story-length estimate. Label estimates and the metadata source. |
| Back to your childhood | Choose a year range, systems or a personal list; generate a nostalgia shelf across all available editions. |
| Franchise journey | Release-order or story-order playlists, with manually curated exceptions and separate completion progress. |
| Handheld drawer | GB through Vita/3DS/PSP collections, filtered by the user's actual owned/imported library. |
| Translations to try | Imported patched variants grouped by language, patch author/version and source game. |
| Mod laboratory | Installed mod profiles for Doom, OpenMW, native ports and compatible engines; show conflicts and dependencies. |
| Works on this machine | Filter using a locally tested profile, with hardware/build/date evidence. Upstream reports alone should say “Reported compatible.” |
| Rescue queue | Games blocked by a missing runner, BIOS, unsupported content revision or disconnected drive, grouped by the next useful action. |
| Museum shelf | User-imported prototypes, demos, manuals, maps and historical notes, with clear provenance. |
| Daily cartridge | A deterministic daily choice from playable, unplayed entries, with optional session-length and genre limits. |
| The living room / bedroom / desk | Launch a configured console or PC streaming session from the same library, while showing which machine does the work. |
| Family shelves | Separate favourites, progress, controller profiles and optionally saves per local user. Shared artwork and read-only game data can be reused. |

Shared collections should export IDs, filters and annotations, not game files, firmware, keys, tokens or local filesystem paths. An imported collection can match only games the recipient already has and show unresolved entries separately.

## Other useful integrations

- **Save vault:** integrate [Ludusavi](https://github.com/mtkennerly/ludusavi) for supported PC saves, then add explicit emulator save mappings. Include memory cards and native saves where configured. Keep save states tied to the emulator/core version; never assume they migrate between engines.
- **Achievement shelf:** use [RetroAchievements](https://docs.retroachievements.org/) where the actual emulator, game hash and mode support it. Showing achievements is different from earning them; enabling a port does not automatically enable an equivalent achievement set.
- **NAS library:** connect to an existing [RomM](https://github.com/rommapp/romm) server for metadata and optional on-demand content transfer from the user's collection. Store access credentials in the system keyring and handle offline entries.
- **PC streaming:** [Moonlight](https://github.com/moonlight-stream/moonlight-qt) with a configured [Sunshine](https://github.com/LizardByte/Sunshine) host. Begin with host/session tiles; per-game launch and wake-on-LAN need explicit client/host capability checks.
- **Artwork studio:** editable cover, logo and hero art; local overrides survive rescans. Let users switch between original box art and a uniform console-style presentation.
- **Controller desk:** player order, button-layout glyphs, connection status, per-runner mapping instructions and a test screen. Motion controls, touch screens, light guns and steering wheels should have their own capability tags.
- **Manuals and extras:** attach local PDFs, maps, soundtracks, screenshots and readme files. A controller-accessible manual view can be part of a later overlay.
- **Compatibility notebook:** local notes, known-good runner builds, selected patches and an exportable diagnostic summary stripped of secrets and personal paths.

## Starter content that can actually be offered

For a first-run demo shelf, evaluate [Freedoom](https://freedoom.github.io/), games on [ScummVM's own download page](https://www.scummvm.org/games/), and [MAME's permission-based ROM download list](https://www.mamedev.org/roms/). Track the license and redistribution conditions for each exact item. A title being free to download does not automatically permit bundling it in Marquee.

Commercial ROMs, BIOS images and proprietary game assets should be user-supplied inputs. The existing roadmap already makes Marquee a frontend over tools and excludes game-data downloads; a curated collection should primarily install recipes, metadata and runners.

## Data model and implementation plan

Keep these concepts distinct in the future core:

| Entity | Purpose |
| --- | --- |
| Game | A title/franchise metadata identity. |
| Edition | Original platform, region, revision, language and content hash/serial. |
| Installation | Local data path or configured remote target, availability and validation status. |
| Runner | Emulator, core, native engine, store client or streaming client, including its actual version. |
| Launch profile | Installation + runner + arguments/configuration + controller mapping + mod list + save location. |
| Compatibility report | Game revision, runner version, OS/hardware, result, evidence URL and tested date. Unknown is a real state. |
| Collection | Saved rules and/or manual membership referencing games/editions/profiles. |

Expand the current narrow TypeScript `Platform` list when these real adapters are added. Add a runner registry and system descriptors in the planned `catalog/systems/`; do not encode every platform's setup in Svelte screens. Port recipes should separately record `engine_installed`, `content_validated` and `launchable` rather than inferring readiness from a completed binary download.

Compatibility and installation are separate axes. Suggested readiness states are: missing runner, missing firmware, missing content, unsupported revision, offline volume, untested, ready, running and failed. Experimental is a property of a runner/profile, not a synonym for installed.

When launching, pass an executable and argument array rather than constructing a shell command. Prefer the user's existing runner installations first, and make Marquee-managed installs explicit. Pin a known working version per profile and retain a rollback path before updating. Preserve the original data and back up settings that an adapter changes.

Recommended sequence:

1. **Real library foundation:** Tauri/core/CLI, Steam and Lutris discovery, explicit process tracking. Acceptance: imported entries launch and record actual playtime.
2. **First console shelf:** DuckStation, PCSX2, PPSSPP, Dolphin and mGBA adapters. Acceptance: prerequisites, supported media, exit/return and save locations work for a tested title per system.
3. **Native revival shelf:** the first ten port recipes listed above. Acceptance: correct input revisions, separate content readiness, reproducible installation and an actual launch.
4. **Collections and editions:** system shelves, franchise grouping, favourites, multiple launch choices and the rescue queue. Acceptance: grouping never loses independent installs or saves.
5. **Broader console generation:** RPCS3, DS/3DS, Wii U, Saturn, Dreamcast and original Xbox, with runner-specific setup.
6. **Experimental lab:** shadPS4, Vita3K, Xenia Canary and KytyPS5 profiles. Acceptance: named build/title/hardware records; unknown or failing games are not advertised as ready.
7. **Connected collection:** chiaki-ng, Moonlight, RomM and save backups. Acceptance: pairing/credentials, offline behavior and host-versus-client execution are clear.

The most valuable first experience is a small shelf that reliably launches the user's games and returns to Marquee. The catalogue can grow independently once those adapter contracts are proven.
