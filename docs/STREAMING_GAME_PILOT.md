# Sonic virtual-desktop pilot

Updated 2026-10-24. Operator-driven ST2b contract for the [streaming plan](STREAMING.md).
This is not automatic hosting UI or an embedded receiver. ST2c remains separate.

## Native build

```sh
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming game-pilot-build --blastem-dir /absolute/path/managed/blastem/current
```

The command composes the current immutable managed Wolf image with the reviewed
BlastEm standalone image at source revision
`ab743653ed110eeb7b1fe64d00cc5f9279c2bfec`. It checks rootless Podman, bounds
input receipts, rejects mismatched image/source records and probes the resulting
emulator before atomically publishing `streaming/sonic-pilot/current.json`.
The receipt retains both input image identities and the recipe hash; its build
log is alongside it. Failed builds preserve the published receipt and old images.

The recipe in `catalog/streaming/sonic/` copies the compiled emulator, assets and
license and installs Fedora runtime libraries. No game data enters the image.
It does not invoke the existing host-desktop BlastEm wrapper, nest Podman, open
the library database or start hosting. OS packages remain moving build inputs;
the local image is immutable, not a bit-for-bit reproducibility promise. Full
distribution review remains open; BlastEm's license is at `/opt/blastem/COPYING`.

## Explicit runtime contract

The operator starts only the owned pilot container with these narrow mounts:

| Input | Container destination |
| --- | --- |
| Private pilot `streaming/wolf/state` | `/state`, writable; contains this pilot's identity and config |
| One reviewed existing Sonic file | `/games/sonic.md`, read-only; never mount the parent collection |
| Private, user-owned `state/game-config` | `/home/retro/.config`, writable; this is a container path, not a host-home mount |
| Saves and diagnostics | `/state/game-saves/sonic`; independent of personal saves |
| Cache | `/state/game-cache`; created by the game launcher |

Use rootless `--userns=keep-id`, `--pull=never`, dropped capabilities and
no-new-privileges. Preserve the base image's unset user declaration: an explicit
root user changes keep-id behavior and blocks private-state access. Do not loosen
state permissions to compensate. No host IPC, host desktop sockets, home, input
device directory or container-management socket is mounted.

Use the configured NVIDIA CDI mapping and selected render node, explicitly
`WOLF_RENDER_NODE`/`WOLF_ENCODER_NODE` on this machine. `/dev/dri/renderD129` is a
local observation, not a portable default. Publish TCP 47984/47989/48010 and UDP
47999/48100/48200 to `127.0.0.1` only, as in ST2a.

The entrypoint adds the **Marquee Sonic pilot** process app to the dedicated
single-profile host. It preserves identity, paired clients, the synthetic app
and encoding settings. Wolf rewrites configuration using inline TOML after
pairing; the pilot supports the reviewed primitive types, verifies a lossless
round trip before atomic replacement, and refuses incompatible apps, unrelated
catalogues and unsupported values. Interrupted staging is preserved.

Wolf supplies a private Wayland socket and per-session PulseAudio sink/server.
The launcher validates these, runs SDL Wayland/software rendering with PulseAudio,
and redirects save-state, SRAM, screenshot and VGM output to the test folder.
An exclusive nonblocking writer lock prevents concurrent sessions sharing saves.
`last-launch.log` and `last-exit.json` distinguish game diagnostics/return status
from receiver lifetime; abrupt group termination can prevent an exit record.

Connect the isolated Moonlight reference receiver to the app at 720p/60 H.264.
Keyboard: Enter, arrows, A/S/D (or W where accepted); backtick saves and L loads.
The pilot also adds **F6 save / F9 load** without removing the existing bindings.

## Live evidence

On the owner's Fedora 9950X3D/RTX 5090 machine, Wolf created `wayland-1` and BlastEm
created a 1280×720 window. The owner confirmed Sonic video, game audio and
forwarded keyboard play. The first accepted stream ran about 69 seconds, reporting
60.00 FPS received/decoded/rendered, 0% network/jitter drops and roughly 1 ms local
latency. No physical desktop capture was used.

Closing that stream also ended the host game/session; the exact close action was
not established. This is **not** proof of reconnect to a retained game. A later
save/restore attempt created a 141,640-byte state file, but the owner reported no
visible restoration with backtick/L. After adding alternate F6/F9 bindings, the
owner explicitly confirmed restoration of the saved position, then confirmed
backtick/tilde-key plus L restoration after reconnect too. Retain both bindings;
this is not certification of every keyboard layout.

A controlled receiver-loss test subsequently passed: the harness started its own
Moonlight process group, terminated only that receiver, waited 12 seconds, and
reconnected. Both the host BlastEm PID and Wolf session ID remained identical
before loss, after loss and after reconnect; no second game/save writer appeared.
Moonlight resumed decoding/audio packets, and the owner confirmed reconnected
video/audio/controls and normal game exit. BlastEm returned **0**, the game process
disappeared and Wolf reported **zero sessions**. The state file persisted. The
owned pilot host was stopped cleanly afterward.

ST2b and the keyboard/save/receiver-loss/exit portion of ST2c passed. Controller
forwarding, focus/input isolation, ordinary receiver-window disconnect semantics,
the native hosting UI and second-device/LAN acceptance remain separate. The
earlier close action ended the game and is not relabelled as a retained session.
Do not mark full ST2 complete from the successful ST2b launch.

Evidence stays in ignored pilot directories. No game, executable image, pairing
identity or private save is committed. Full repository checks and catalogue/config
preservation tests cover the implementation, not the remaining human acceptance.
