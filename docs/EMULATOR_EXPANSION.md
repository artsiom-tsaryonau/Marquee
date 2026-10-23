# Collection-driven emulator expansion

Updated 2026-10-24. This is the next-work plan for the owner's existing collection,
alongside [streaming](STREAMING.md). The [emulator matrix](EMULATORS.md) remains
authoritative for implemented adapters and title-specific acceptance. A catalogue
entry, installed executable and working game are three different states.

## What is already available

A read-only inventory found managed BlastEm, DuckStation, FinalBurn Neo, Flycast,
melonDS, Mesen, PCSX2 and RMG executables registered in the local library. Their
presence does not establish game compatibility or streamed input readiness.

The active `roms` folders contain these candidate inputs. Counts are files, not
validated titles; removed content and non-game artwork/source files are excluded.

| Cohort | Existing inputs | Next useful work |
| --- | --- | --- |
| Genesis | 64 BIN, 5 SMD, 4 MD | Stream the locally accepted Sonic/BlastEm path first; BIN needs platform-aware inspection |
| Mesen systems | NES 239 NES files plus 56 ZIPs; SNES 38 SMC/SFC; GB 22 GB; GBC folder 457 GB/GBC; PCE 6 files under `other` | One representative title per platform: local launch, input, native saves, state restore where supported, normal exit; then stream the same profile |
| Nintendo DS | 1 NDS | Validate touch/pointer plus buttons and save paths in melonDS; streaming must preserve both screens and pointer coordinates |
| Neo Geo | 1 ZIP | Verify complete matching ROM set and required BIOS before FinalBurn Neo acceptance; archive presence alone is insufficient |
| Neo Geo Pocket | 11 NGP files under `other` | Missing library adapter and managed runner recipe; inspect headers, choose and pin a suitable backend, then validate one title |
| Dreamcast / N64 | 3 CDI; 4 N64/Z64 | Existing Flycast/RMG paths need representative input, saves and streamed acceptance; keep controller mapping explicit |
| PS1 / PS2 | CHD, ISO and paired CUE/BIN tracks; 3 PS2 ISOs | Continue existing disc paths; track files are not separate titles; larger-system work stays independent |

These observations describe this machine, not distributable starter content.
Do not move files, resurrect `_removed`, or register every matching extension
without inspection. Files in `other` need explicit platform review.

## Implementation order and gates

1. **E0 — Shared streaming baseline.** Synthetic video/audio and Sonic video,
   audio, keyboard play, state restoration, controlled receiver-loss/reconnect and
   game exit in Wolf's virtual desktop passed. Complete physical controller,
   focus isolation and ordinary viewer-window semantics. Local Sonic acceptance
   alone does not satisfy this gate.
2. **E1 — Existing lightweight adapters.** Mesen NES → SNES → GB/GBC → PCE,
   followed by melonDS and FinalBurn Neo with valid data. Reuse installed runners
   and shared profiles; fix observed launch/save/input problems before adding
   another engine. Record exact title, runner build and tested capabilities.
3. **E2 — Neo Geo Pocket coverage.** Add the missing descriptor, reviewed source
   build/install recipe, format validation, save mapping and input instructions.
   Select the backend after source/license/capability review. Test one current
   input locally and through streaming before claiming the platform works.
4. **E3 — mGBA and PSP when useful.** An mGBA source recipe already exists, but
   it is not a registered library launch path. Add explicit alternative runner
   selection for GB/GBC and a GBA descriptor; do not silently replace Mesen.
   No loose GBA or PSP sample was found in this inventory, so actual game
   acceptance needs a suitable input. PPSSPP's tool entry alone is insufficient.
5. **E4 — Broader systems.** Complete Dreamcast/N64 mapping and acceptance, then
   follow the separate [console preparation plan](CONSOLE_PREPARATION.md) for
   package-based systems. PS4/PS5 installation is not implied by torrent success.

For every cohort, track `installed`, `local launch tested`, and `stream tested`
separately. Preserve per-edition identity, explicit BIOS requirements, dedicated
saves and runner-version-bound save states. Keep game data read-only when possible
and prevent simultaneous writers to personal saves. Local and streamed profiles
must agree on save ownership without nesting incompatible container launchers.

Completion requires real video/audio, the intended input device, save persistence
after restart and normal exit. Streaming also requires focus/input isolation,
pointer/controller behavior, disconnect/reconnect and no recursive capture.
Do not treat a process spawn or upstream compatibility report as that evidence.
