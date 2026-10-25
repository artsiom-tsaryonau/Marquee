# Private-display streaming for the library

Updated 2026-10-25. The target is one streaming launch option for every supported
emulator and native port. A graphical emulator still renders to a private virtual
display; its window does not appear on the physical desktop. This is distinct
from an emulator's batch/testing mode, which may have no playable video or input.

## Shared contract

Use Wolf's independent compositor and audio route, with Marquee owning session
selection and the receiver. Native Wayland applications connect directly. X11
applications run under a nested Gamescope/XWayland bridge. Never capture the
physical desktop or mount its display/audio sockets as a shortcut. Wolf documents
this [XWayland architecture](https://games-on-whales.github.io/wolf/stable/dev/how-it-works.html).

Each adapter must supply a reviewed executable and argument vector, its runtime
libraries, read-only game inputs, narrowly scoped firmware where required, and a
persistent writable emulator profile. Keep one writer per profile. Receiver
closure must not silently destroy the game or its saves. Returning to a stream
must reconnect to the existing process. Explicit game exit ends the host session.
Local and streamed profiles must not write the same saves concurrently.

A generic `--headless` argument is not portable. Current local launch capability
also does not prove streaming capability. A runner becomes selectable for
streaming only after video, audio, forwarded input, exit, save and reconnect gates
pass. Keep normal Play available throughout the rollout.

## Rollout

| Runner family | Display path / work | Accepted streaming evidence |
| --- | --- | --- |
| BlastEm / Genesis | Existing reviewed source image; native SDL Wayland | Sonic video/audio/keyboard, state restore, reconnect and normal game exit |
| Xenia / Xbox 360 | Pinned Linux build; nested Gamescope/XWayland; isolated content/cache/config/log paths | New Vanquish pilot below; separate acceptance required |
| Mesen / NES, SNES, GB, GBC, PC Engine | Package the existing runtime and review its display/input backend | Local NES/SNES acceptance only; streamed validation pending |
| DuckStation / PS1 and PCSX2 / PS2 | Reuse managed builds; private Qt display, required BIOS and persistent cards | Pending |
| RPCS3 / PS3 | Managed build plus installed firmware/profile; private Qt display and title data | Local God of War III menu acceptance only; streaming pending |
| Flycast, melonDS, RMG and FinalBurn Neo | Package each actual runtime, required BIOS and its input configuration | Pending |
| Shipwright and other native ports/recomps | Runtime-specific SDL/display route and original/generated assets; no engine-only images claiming playable games | Pending |
| xemu / Xbox | Configure and validate BIOS/MCPX/mutable HDD and disc adapter first | No library Play adapter yet |
| shadPS4 / PS4 | Title-specific local compatibility first, then private-display packaging | Shadow of the Colossus remains unplayable; streaming cannot fix missing emulation |
| PS5 research runtimes | Actual supported game/runtime contract first | No validated library launch or streaming adapter |

After the X11 bridge is accepted, migrate Mesen and the Qt runners. Then replace
pilot-specific game slots with library-owned launch plans: select edition + saved
runner, validate media/firmware, acquire the profile writer lease, start/reuse the
owned backend, publish one app, open/reconnect the viewer, and reconcile exit.
Expose Local / Stream beside Play only when the selected runner has an available
streaming adapter. Existing streaming UI lists host-configured apps today; this
library-wide orchestration is not implemented by the pilot.

## Xenia acceptance pilot

The explicit helper is `scripts/streaming-xenia.py`. It builds on the immutable
reviewed Sonic/Wolf image and downloads the same SHA-256-pinned Xenia release as
the managed local adapter. Game data, firmware and personal profiles are not baked
into the image. The image records a package/license inventory and includes Xenia's
license. Fedora package versions are recorded build inputs, not reproducible pins.

Gamescope's optional `cap_sys_nice` file capability is removed inside the image;
the rootless runtime retains dropped capabilities and no-new-privileges. It
publishes only loopback ports, uses the selected NVIDIA CDI/render device and
mounts the reviewed Vanquish ISO read-only. No host display, home, input-device
directory, container socket or physical audio socket is mounted.

```bash
python3 scripts/streaming-xenia.py build
python3 scripts/streaming-xenia.py start \
  --iso /absolute/path/Vanquish.iso \
  --render-node /dev/dri/renderD129 \
  --sonic /absolute/path/previous-pilot-sonic.md
# Stop only this helper's owned container:
python3 scripts/streaming-xenia.py stop
```

The render node above is this machine's observed NVIDIA node, not a portable
default. `--sonic` preserves the earlier configured Sonic app when present.
The supplied ISO is inspected before starting and must have Vanquish title ID
`53450841`. This is a bounded acceptance slot, not arbitrary-library streaming.
The host retains its existing pairing/configuration. Its added app is **Marquee
Vanquish pilot**. It refuses an unrelated catalogue instead of replacing it.

Test saves/configuration live under
`data/streaming-pilot/streaming/wolf/state/game-saves/xenia`, separate from the main
library's Xenia profile. Enter is Start, Z is Back, J/K/U/I are A/B/X/Y, WASD moves,
arrows look and Q/E are triggers. `last-launch.log` and `last-exit.json` record
runtime evidence. A build receipt's `live_streaming_verified: false` must not be
interpreted as acceptance.

## Artwork and launch selector correction

Preparation registration now starts the same cover/facts refresh as regular
imports. The desktop allows only the current artwork cache tree and the
instance-local `cache/art` tree, so CLI-prepared covers remain visible when the
desktop uses XDG cache paths. The authenticated browser artwork route has the
same fallback. No whole-library or home-directory asset permission is granted.

Vanquish already had a downloaded cover, description, genre, year and player
count; the mismatched cache permission hid its cover in the desktop. A provider
search can still require release review, and homebrew games need not have retail
artwork. The selector now lists each runner once, marks the default with
`(default)`, and uses a styled keyboard-accessible popup. Selecting that row
clears a game override; an unavailable saved override remains visible.

Live acceptance follow-up: the native Gamescope Wayland backend failed to create
a Vulkan surface on the NVIDIA host. The working route uses Gamescope's SDL
backend with `SDL_VIDEODRIVER=wayland`; only the Xenia child receives
`GDK_BACKEND=x11`. The owner confirmed streamed video, keyboard input and audio.
The initially reported silence was the receiver's selected physical output, not
a silent private game sink. No global audio output change or permanent receiver
driver override was applied. Closing the receiver retained the same Xenia PID;
reopening attached to that session. Gameplay/checkpoint and full save/relaunch
validation remain separate gates.

Browser verification confirmed Vanquish's cover actually decoded, its description
appeared, one default emulator row was present, Escape returned focus to the
trigger, and the 390 px page had no overflow or JavaScript errors.
