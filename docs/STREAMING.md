# Native streaming integration

Updated 2026-10-23. The owner scheduled this track and authorized selective source
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

| Step | Work | Gate |
| --- | --- | --- |
| ST1 | Reproducible managed Wolf source build and runtime probe | Pin dependencies/base images; audit bundled licenses; build and launch under reviewed Podman permissions; identify GPU/encoder and socket capability; stop/rollback cleanly |
| ST2 | Isolated Sonic/BlastEm streaming pilot | Reference Moonlight receiver on this machine; virtual Wayland display/audio; video, streamed keyboard/controller, audio, save-state restoration, disconnect/reconnect and clean exit observed |
| ST3 | Native Marquee hosting interface | Pair/revoke clients, show stream quality/state and reconnect/stop actions; map Wolf sessions to library editions and actual runner lifetime; desktop/browser parity without exposing the raw privileged API |
| ST4 | Marquee receiver | Choose native decoding/rendering design, audit licensing and prove latency/input/focus/audio behavior; playback within Marquee needs a receiver implementation, not just a web video element |
| ST5 | Additional hosts and devices | Sunshine/Apollo adapters with capability-specific controls, second-device/LAN acceptance, wake-on-LAN and remote trust/recovery |

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

ST1 must also review upstream logging: the pinned API implementation can log
request bodies at DEBUG level. Marquee's redaction does not sanitize Wolf's own
logs. Managed defaults/patches must prevent PINs and session keys reaching normal
support bundles. Keep Wolf's command-capable API on a protected local socket.

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
