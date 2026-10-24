# Native streaming integration

Updated 2026-10-24. The owner scheduled this track and authorized selective source
reuse or replacement where useful. Marquee owns the interface and session model;
existing streaming engines supply capture, encoding and transport initially.

## Project roles and backend choice

| Project | Role | Intended integration |
| --- | --- | --- |
| Sunshine | General desktop/game streaming host | Later backend for an existing host desktop |
| Apollo | Sunshine fork with client permissions and automatic Windows virtual displays | Later Windows backend; its built-in virtual display is not implemented for Linux |
| Wolf | Linux host with on-demand independent Wayland desktops, virtual audio/input and Moonlight transport | First backend for isolated Linux game sessions |
| Moonlight | Receiving client: decode/render video, play audio and send input | Reference receiver for acceptance; native receiver integration is a separate milestone |

Wolf creates a compositor session; Apollo's Windows display driver creates a
virtual monitor. Those are different mechanisms. Same-machine testing can run
the emulator in Wolf's separate desktop and show the receiver on the physical
desktop, avoiding recursive desktop capture. Correct input isolation must still
be observed, not inferred from the presence of a virtual display.

Primary sources: [Wolf architecture](https://games-on-whales.github.io/wolf/stable/dev/how-it-works.html),
[Wolf API](https://games-on-whales.github.io/wolf/stable/dev/api.html),
[Sunshine](https://github.com/LizardByte/Sunshine),
[Apollo](https://github.com/ClassicOldSong/Apollo),
[Moonlight PC](https://github.com/moonlight-stream/moonlight-qt), and
[Moonlight client core](https://github.com/moonlight-stream/moonlight-common-c).

## ST0 — Source contract and native control adapter

Implemented: `marquee_core::streaming::wolf::WolfClient` and the CLI `streaming`
group. The adapter is based on Wolf revision
`2d2ae0021ad812f79a966dd10e8289e0fae98ecb` from its active `beta` branch.
The upstream `stable` Git branch is a 2024 revision without this control API.
The reviewed commit identifies the contract; it is not a detected live version.
See [source provenance, schema and MIT license](../third_party/wolf/README.md).

- Read advertised apps, paired clients and active sessions using API v1.
- Show pending client addresses; accept a four-digit PIN for exactly one matching
  request. The pending secret never leaves the adapter. An ambiguous address
  requires the user to resolve the duplicate request in the client first.
- Explicitly revoke a paired client or request that a selected session stop.
- Use only an explicitly supplied absolute Unix socket; no TCP proxy, discovery,
  backend start, port opening or automatic runtime installation.
- Bound response headers to 16 KiB and bodies to 2 MiB; enforce response deadlines,
  reject unsupported framing, redirects, malformed contracts and false success.
- Project only needed fields. Drop session encryption keys, pairing secrets,
  runner commands, pipelines and client state paths from public output. Backend
  error bodies are never copied to diagnostics.
- Operate without opening/creating `library.db`, starting the manager or restoring
  transfers, including while the desktop owns the library lease.

The upstream session reflector serializes its session ID as `client_id`. Marquee
returns it as `session_id`; `/clients` still returns actual paired-client IDs.
Inspect performs separate reads and does not report an atomic snapshot, verified
GPU capability, or successful game readiness. A successful pairing POST means
the PIN was accepted, not that the client's certificate handshake completed.
A stop acknowledgement is a request, not proof of process exit. Upstream's stop
can terminate the game; it must never be presented as ordinary viewer disconnect.

CLI against an independently running compatible Wolf host:

```sh
cargo run -p marquee-cli -- streaming --socket /absolute/path/wolf.sock inspect
cargo run -p marquee-cli -- streaming --socket /absolute/path/wolf.sock pending
read -r -s -p 'Moonlight PIN: ' marquee_stream_pin
printf '%s' "$marquee_stream_pin" | cargo run -p marquee-cli -- streaming --socket /absolute/path/wolf.sock pair --client-ip 127.0.0.1
unset marquee_stream_pin
cargo run -p marquee-cli -- streaming --socket /absolute/path/wolf.sock unpair CLIENT_ID
# Explicitly ends the selected session and may terminate its game:
cargo run -p marquee-cli -- streaming --socket /absolute/path/wolf.sock stop SESSION_ID
```

ST0 verification uses synthetic, source-derived Unix HTTP fixtures, the retained
upstream OpenAPI schema, timeout/truncation cases and a real CLI with an occupied
library lease. It is **not live Wolf streaming acceptance**. No backend, receiver,
virtual display or game launch has been installed or enabled by ST0. These
controls are not yet exposed through the desktop/browser API.

Verification on 2026-10-23: all eight new streaming tests passed, followed by the
complete `scripts/check.sh` pass (Rust formatting, workspace Clippy/build/tests,
Tauri compile, Svelte checks/tests and production UI build). Unix-socket tests
require the normal host execution context; the restricted command sandbox blocks
socket creation. Source schema/license comparison and CLI help checks passed.

## Next milestones and completion gates

### ST1 local source build and host probe — implemented

The native CLI builds Wolf and its Rust Wayland compositor in rootless Podman.
The recipe is in `catalog/streaming/wolf/`; root Git revisions, the compositor's
Cargo lock, CMake dependency revisions and base-image digests are pinned. Resolved
OS package versions/licenses and Rust dependency metadata are retained in the
image. Fedora repositories remain moving inputs: the receipt identifies an
immutable local image, not a bit-for-bit reproducible rebuild.

```sh
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming build
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming runtime-status
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming probe
# Requires an existing NVIDIA CDI mapping; render device numbers are host-specific.
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming probe --nvidia --render-node /dev/dri/renderD129
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming rollback
```

Builds are synchronous CLI operations, separate from durable manager jobs. Each
stage reuses the existing bounded/cancellable process runner. The adapter accepts
a cancellation flag; detached recovery and UI cancellation remain open. Logs are
in `<data>/streaming/wolf/build.log`. Build/probe must pass before `current.json`
is atomically replaced; `previous.json` retains the previous image. Rollback
probes that image before selecting it. Images, private state and configuration
are retained. These commands do not open the library or launch games.

The image entrypoint supervises private PulseAudio and Wolf processes. It creates
a persistent private host identity/configuration and one synthetic app. It does
not start the upstream Wolf UI, mount a container-management socket or include
games. Source patches remove dynamic API/query/error log fields and pairing
secrets, and replace an empty audio-server dereference with an explicit failure.
CUDA is enabled in both compositor crates; the fake-udev helper's static C/C++
link dependencies are included. Corrections belong to the reviewed recipe.

Local acceptance on 2026-10-23, targeting the owner's Ryzen 9 9950X3D and RTX
5090 (observed NVIDIA driver 615.71.09):

- Native build completed. The no-GPU probe loaded compositor/Opus factories and
  performed software H.264 encoding. NVIDIA CDI plus this host's `renderD129`
  performed a real NVIDIA H.264 encode. Factory presence alone does not satisfy
  the selected NVIDIA probe.
- A rootless container with private state and NVIDIA access started its audio
  server and Wolf. All six published TCP/UDP ports bound to `127.0.0.1` only.
  No home, games, input devices, host IPC or container socket were mounted.
- Marquee read the real app/client/session and pending-pair APIs. Only the
  synthetic test app was advertised.
- Both starts ended with Wolf's `Graceful shutdown complete` marker. Private
  key, certificate and configuration hashes stayed identical across restart.
- A metadata-only update produced another image ID; native rollback selected the
  previous image, and a second rollback restored the update. This verifies receipt
  promotion mechanics, not cross-release configuration migration.

Evidence stays under ignored `data/streaming/` and `data/streaming-pilot/`.
Pilot containers were stopped; the image receipt remains for ST2. Startup/stop
was an operator-driven acceptance run. Automatic backend lifecycle ownership,
desktop hosting UI, packaged installation and full third-party redistribution/
source-offer review remain open. The license inventory supports that review.

**At the end of ST1, still unverified:** streamed video/audio, a virtual desktop rendering a game,
client pairing/handshake, streamed input and disconnect/reconnect during play.
No Moonlight receiver was installed by ST1. ST2 is the first end-to-end stream gate.

Verification: the full `scripts/check.sh` pass includes source-patch regression,
Rust workspace/CLI tests, Clippy, desktop compile and Svelte tests/build. Final
receipt-version/validation changes were rechecked with Clippy, targeted tests and
a CLI rebuild; the rebuilt CLI also read the existing receipt and passed the GPU
probe. Local documentation links and whitespace checks passed.

| Step | Work | Gate |
| --- | --- | --- |
| ST1 | Local source build and host probe implemented | Source/image pins, software/NVIDIA encoding, real API, restart/stop and receipt rollback passed; OS snapshots, distribution review and packaged lifecycle remain open |
| ST2 | Fedora-to-itself streaming pilot — keyboard/mouse path tested | Synthetic/Sonic video/audio, keyboard/focus, save restoration, receiver-loss/reconnect, explicit viewer disconnect and game exit passed; controller deferred for hardware, window-close button and automatic focus return remain open |
| ST3 | Streaming screen implemented for an existing local host | Desktop/browser status, viewer install/open/close, pairing, revoke and confirmed host-session stop share one core API. Host startup, edition mapping, measured quality and automatic focus return remain open |
| ST4 | Marquee receiver | Choose native decoding/rendering design, audit licensing and prove latency/input/focus/audio behavior; playback within Marquee needs a receiver implementation, not just a web video element |
| ST5 | Second-device/LAN acceptance, then additional hosts | Separate Fedora or other supported receiver; real network latency/bandwidth/input/reconnect checks; Sunshine/Apollo adapters and wake-on-LAN follow capability-specific gates |

## ST2 — Fedora-to-itself streaming

### ST2a progress — 2026-10-24

The optional native `streaming receiver-install` command now downloads the pinned
official Moonlight Qt **6.2.0 Linux x86_64 AppImage**, verifies its published SHA-256
and exact size before executing extraction, checks executable containment, and
promotes a versioned program directory with a bounded validated receipt. A failed
download/extraction does not publish an installation. Re-running reuses the
validated receipt; it does not overwrite preferences or pair/start a receiver.
`receiver-status` reads this state independently of the library database.

```sh
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming receiver-install
cargo run -p marquee-cli -- --data-dir /absolute/path/streaming-pilot streaming receiver-status
```

The manifest is `catalog/streaming/moonlight/reference.json`. It retains official
binary and matching source URLs/checksums and GPL-3.0 identification. This is an
optional downloaded reference program, not an embedded player or a completed
redistribution/license audit. It extracts without requiring a FUSE mount.

Live observations on the owner's Fedora 9950X3D/5090 machine:

- Official receiver download/checksum/extraction and managed installation passed.
- This AppImage contains the Qt `xcb` plugin, not the Wayland plugin. The UI runs
  under XWayland with `QT_QPA_PLATFORM=xcb`; do not force Qt's `wayland` platform.
- Separate XDG config/data/cache directories keep pilot pairing/preferences apart
  from any existing Moonlight setup. The host identity and private state remain
  under the dedicated pilot data directory.
- PIN pairing through Marquee completed and Wolf registered the client. Rootless
  networking reported the client's host address rather than `127.0.0.1`; select
  the actual pending request instead of hardcoding the pairing address.
- At 720p/60 H.264, Moonlight received/decoded/rendered **59.99 FPS**, reported
  0% network/jitter drops and roughly 1 ms network latency. It used NVIDIA VDPAU
  after VAAPI initialization failed. These are same-machine diagnostic readings.
- The first run displayed video but the owner heard no audio before termination.
  After the readiness fix, the owner explicitly confirmed **sustained moving
  video and audible ticking audio**. Moonlight used PipeWire and its stream was
  routed to the existing desktop output; desktop audio settings were not changed.
- The first stream terminated after five seconds because the pinned Wolf source
  waited for an empty Wayland socket despite `start_virtual_compositor=false`.
  The strict source patch now guards display creation/readiness for compositor
  apps only. The rebuilt host passed the sustained playback retest using the
  managed receiver and preserved client pairing across the host restart.
  The retest ran for approximately 42 seconds until the requested stop, with
  60.00 FPS received/decoded/rendered and 0% reported network/jitter drops.

**ST2a passed.** Marquee requested the synthetic session stop after acceptance and
the owned pilot host was stopped cleanly. Full repository checks, source-patch
regression and receiver checksum/containment tests passed. Evidence remains in
ignored pilot directories; no receiver binary, pairing identity or game data is
committed. This evidence does not establish a virtual game desktop,
streamed keyboard/controller support, game audio, saves or reconnect acceptance.

### ST2b progress — 2026-10-24

The [Sonic game pilot contract](STREAMING_GAME_PILOT.md) now composes the managed
Wolf and BlastEm images through native `streaming game-pilot-build`. It runs the
emulator directly in Wolf's private Wayland/audio session, mounts one game
read-only, and keeps test configuration/saves separate. The owner confirmed real
game video, audio and forwarded keyboard controls. **ST2b launch passed.**

ST2c remains open: the first attempted viewer close also ended the game, so it
does not establish reconnect. Subsequent acceptance confirmed **F6/F9** restoration
and backtick/tilde-key plus L restoration. A controlled receiver-process loss and
reconnect retained the exact BlastEm PID and Wolf session ID; the owner confirmed
reconnected video/audio/controls. Normal game exit returned 0 and cleared the game
process and Wolf sessions. The state file persisted and the owned host was stopped.

The keyboard/save/receiver-loss/exit path passed. Subsequent owned-viewer tests
below confirmed focus/key release, mouse release and explicit disconnect paths.
Physical controller, the OS window-close button and automatic focus return remain
pending. The earlier ambiguous close is not relabelled as a retained session.
The pilot remains operator-driven, not
ST3 lifecycle/UI. See the [evidence and limitations](STREAMING_GAME_PILOT.md#live-evidence).

Owner clarification, 2026-10-24: no physical controller is available yet; it has
not been purchased. Defer that acceptance gate until hardware is available.
Proceed with keyboard/mouse focus, viewer-disconnect behavior and ST3 native UX;
controller testing is not a prerequisite for those increments. Keep physical
controller support explicitly unverified. See [controls planning](CONTROLLERS.md).

### Native viewer ownership and focus — 2026-10-24

The [owned reference viewer](STREAMING_VIEWER.md) adds native CLI open/status/
disconnect controls with private settings, an exclusive lease, bounded local
control messages and owned-child cleanup. Disconnect never calls host stop.
Process liveness is not reported as verified stream readiness. The owner confirmed
Alt+Tab/key release without stuck input, Ctrl+Alt+Shift+Z mouse release and
Ctrl+Alt+Shift+Q normal receiver disconnect. The shortcut returned 0 while the
same Sonic process stayed alive. Native `disconnect-viewer` and owner-crash/
stale-socket recovery also retained the same game. Full checks and five viewer
ownership/protocol tests passed. This groundwork now backs the [Streaming screen](STREAMING_VIEWER.md#streaming-screen).
No embedded decoder ships; automatic focus return and the OS window-close button
remain separate checks.

The first end-to-end test uses the same Fedora machine as both host and receiver:
Ryzen 9 9950X3D and RTX 5090. Connect Moonlight manually to `127.0.0.1`, using the
existing loopback-only pilot ports. Keep the receiver on the physical desktop;
the game belongs in Wolf's independent virtual desktop. This avoids capturing
the receiver window recursively and makes an extra monitor unnecessary for the
intended test. Native playback inside Marquee remains ST4.

```text
Sonic / BlastEm in Wolf's virtual Wayland desktop
    → NVIDIA H.264 encoding → local connection → Moonlight on Fedora's desktop
    ← keyboard/controller input forwarded through the receiver ←
```

Complete these increments in order and retain separate evidence:

| Increment | Work | Acceptance |
| --- | --- | --- |
| ST2a | Reference Moonlight setup and synthetic loopback stream | Install a reviewed receiver build; pair through Marquee; stream the existing test pattern and tick audio at an initial 720p/60 H.264 baseline; confirm real decoding/display/audio and paired-client persistence |
| ST2b | Sonic virtual-desktop launch contract | Run the known-working BlastEm source build against a read-only game input and fresh dedicated saves in Wolf's compositor/audio environment; show the game through Moonlight without capturing the host desktop |
| ST2c | Streamed gameplay and lifecycle acceptance | Demonstrate forwarded keyboard and controller input separately, save/restore, audio, viewer disconnect/reconnect and normal game exit; reconcile backend/runner lifetime and return to Marquee without disturbing the host desktop |

- [x] Install the receiver; it is not installed by ST1's host build.
- [x] Start the reviewed host with explicit ownership and bounded shutdown;
  preserve identity/configuration. Keep game data and test saves isolated.
- [x] Complete PIN/certificate pairing; confirmed by the live stream, beyond the PIN POST.
- [x] Observe synthetic video/audio, then real Sonic video/audio through the client.
- [x] Verify keyboard input reaches the game through the stream: direct host keyboard
  access must not be mistaken for forwarded input. Track controller acceptance
  separately; working keyboard input does not establish controller support.
- [x] Verify keyboard focus/key release and mouse release on the local receiver.
- [ ] Verify automatic focus return to Marquee and OS window-close behavior.
- [ ] Deferred for hardware: verify physical controller forwarding separately.
- [x] Save a state, change the game position, restore it and observe the result
  in the receiver. Keep this separate from battery-save persistence.
- [x] Define and verify explicit viewer disconnect versus game stop behavior; reconnect to
  the intended session without a duplicate game or competing save writer.
  Controlled receiver loss/reconnect, native `disconnect-viewer` and the normal
  receiver disconnect shortcut passed. Native UI controls and the OS window-close
  button remain separate work.
- [x] Close the game normally and verify runner/session cleanup; stop the owned
  pilot cleanly. Automatic focus return to Marquee remains unverified.
- [x] Record exact host/receiver/emulator revisions, codec, resolution/FPS,
  encoder/decoder, observed audio/input results and available frame/latency stats.

The existing host API, software/NVIDIA encoding probes and local Sonic gameplay
tests do not complete these items. Keep ST2 open until the receiver has actually
displayed the stream and each claimed capability has been observed.

After local acceptance, ST3 can build on the proven contract. ST5 must separately
exercise another device over a real LAN, including bandwidth/latency, packet loss,
input and reconnect behavior. Same-machine testing shares CPU/GPU resources and
does not establish network or second-device decoder performance. Internet access
and wake-on-LAN are additional explicit capabilities; do not widen the pilot's
loopback bindings as part of ST2.

The existing managed BlastEm runner is not automatically a Wolf app. Its Podman
wrapper currently targets the user's native Wayland session. ST2 must provide a
reviewed launch contract for Wolf's compositor, audio, virtual devices, read-only
game input and dedicated persistent save paths. Do not nest incompatible
container launchers, share personal save writers concurrently, or count the
earlier local Sonic acceptance as streamed acceptance.

Core remains authoritative for library identities and gameplay/session accounting.
The hidden manager may build/install/update backends; accepted acquisition jobs
do not own gameplay lifetime. Do not equate a receiver process, pairing POST or
transport connection with game playtime. Keep disconnect, stop stream, and end
game semantics explicit, and reconcile observed backend state after failures.

The pinned implementation can log request bodies at DEBUG level. ST1's patch
removes dynamic API/error/query logs and pairing secrets from the managed build,
in addition to Marquee's response projection. Broader support-bundle review
remains open. Keep the command-capable API on a protected local socket.

## Source reuse and replacements

Wolf's main source is MIT. Sunshine, Apollo and Moonlight's core are GPL-3.0;
audit exact components/dependencies and their distribution requirements before
vendoring, linking or shipping them. An MIT root repository is not a license
inventory for the entire runtime/container. ST0 retains Wolf's license alongside
the adapted contract and unmodified OpenAPI schema; it does not link a decoder.

Marquee can replace orchestration or individual engine components when measured
requirements justify it. Reimplementing encoding, the streaming protocol and
controller transport is not a prerequisite for native UX. Quiver remains a
separate [planning-only reuse track](QUIVER_INTEGRATION.md).
