# Mesen NES and SNES acceptance

Updated 2026-10-24. These are title-specific local tests on the owner's Fedora
machine, using the existing managed Mesen 2.1.1 library adapter. No additional
emulator download or new platform adapter was needed. Streaming acceptance is
separate from these local tests.

## Isolation and runner

A fresh library under `data/emulator-pilot/mesen-20261024` adopted a copy of the
already managed binary and scanned read-only copies of two existing inputs.
Original games and personal saves were left alone. Marquee supplied isolated
XDG directories, its first-run settings seed, `LC_ALL=C.UTF-8` and
`DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1` through the normal catalogue launch
path. Both games launched through the native CLI and its session accounting,
not by opening an emulator manually.

Mesen executable SHA-256:
`ae43f1438282aaaff90a009aa8ada648bc5d631b070656285d7de9cbff513b41`.
Game files, copied binaries, states and local database remain ignored artifacts;
they are not distributed in the repository.

## Confirmed results

| Test | Alter Ego — NES | Uwol: Quest for Money — SNES |
| --- | --- | --- |
| Visible game video | Owner confirmed | Owner confirmed |
| Game audio | Owner confirmed | Owner confirmed |
| Keyboard start and play | Owner confirmed | Owner confirmed |
| Save-state restoration | Not separately tested | Owner confirmed slot 1 restored after moving elsewhere |
| Normal exit | Owner confirmed window close; native exit 0, 40 seconds | Owner confirmed File → Exit; native exit 0, 84 seconds |
| Streamed play | Not tested | Not tested |
| Physical controller | Deferred: no controller available | Deferred: no controller available |

The SNES state exists at the isolated profile's
`mesen/xdg-config/Mesen2/SaveStates/UwolQuestForMoney_1.mss`.
The successful restore session is `3ac3cac9-71e4-4356-9a54-f98ac8e57bb6`;
NES session is `a0578b92-0c84-468e-98db-a61eea07eb11`.

An earlier SNES gameplay session ended before the restore test, with state
`failed` and no numeric exit code after 49 seconds. The exact termination cause
was not established. It is not counted as a successful normal exit; the later
controlled File → Exit run supplies that evidence. A repeat of unexpected window
termination needs diagnosis rather than being treated as played-session success.

## Remaining acceptance

Native battery-save persistence across a fresh process was not demonstrated by
these titles. Do not equate save-state restoration with an in-game save. NES
state restoration and SNES operating-system window-close behavior also remain
separate checks. GB/GBC/PCE coverage is still untested. Neither local test proves
that Mesen renders and accepts input inside Wolf's private desktop: that needs a
reviewed streamed launch profile and its own video/audio/input, save ownership,
disconnect/reconnect and game-exit acceptance. Reuse this managed build and fresh
profiles for that next cohort; do not mark the whole Mesen platform family done.
