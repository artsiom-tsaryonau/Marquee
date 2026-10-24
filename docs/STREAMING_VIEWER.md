# Owned reference viewer and focus

Updated 2026-10-24. ST3 now has a Streaming screen in desktop and browser navigation.
Playback still uses the optional Moonlight reference program in its own window;
there is no embedded decoder.

## Streaming screen

Open **Streaming** in the left navigation. It uses the same authenticated core
commands on Tauri and the browser surface. A browser action opens the native
viewer on the computer running Marquee, not on a remote browser device.

- Connect to an already running local Wolf host. **Advanced connection** selects
  the absolute streaming data folder and Unix API socket; the default is the
  current library data folder. For the existing pilot select
  `<checkout>/data/streaming-pilot` and its
  `streaming/wolf/state/runtime/wolf.sock`. Settings persist per library.
- Install the checksum-pinned reference viewer if absent. **Pair this viewer**
  opens its pairing dialog with the same private preferences used for playback.
  Select the pending device, submit its four-digit PIN, then close the pairing
  window. PIN values are cleared immediately on submission and are not stored.
- **Games offered by host** comes from the live host catalogue. **Open viewer**
  opens or resumes that advertised app. **Close viewer** only disconnects the
  owned receiver; **End game** separately requests session termination and needs
  a second click. Revoking a paired device also requires a second click.
- Status distinguishes an offline host, installed receiver, active operation and
  open viewer. Requested session resolution/FPS is not measured stream quality;
  an open receiver is never represented as verified video/audio playback.
- Navigation away stops screen polling without stopping games or the viewer.
  Overlapping commands/polls are suppressed. Background viewer errors remain
  visible; connection changes are blocked while its ownership lease is active.

Host build/start/stop automation, library-edition mapping, automatic focus return,
measured quality/latency and the embedded receiver remain later work. The current
screen deliberately exposes no nonfunctional host Start button. The existing
operator pilot launcher remains necessary. Physical controller acceptance awaits
hardware. Native and UI tests cover connection validation, ownership conflicts,
background errors, confirmation, command parity and non-overlapping polling.
Live authenticated HTTP acceptance also passed against the existing pilot: offline
default, saved connection, advertised catalogue, viewer open, disconnect retaining
the host session, and explicit end-game clearing it. The isolated host was stopped
afterward. Visual screen verification is pending: no browser surface was available
in the agent session. This does not repeat or replace the earlier human gameplay
acceptance.

## Commands and semantics

```sh
# Install the reviewed receiver separately; this does not pair or start hosting.
marquee --data-dir /absolute/path/pilot streaming receiver-install

# Foreground owner: opens or resumes exactly one advertised app on loopback.
marquee --data-dir /absolute/path/pilot streaming --socket /absolute/path/wolf.sock view --app-id APP_ID

# From another CLI/app process, independent of library.db and the Wolf socket:
marquee --data-dir /absolute/path/pilot streaming viewer-status
marquee --data-dir /absolute/path/pilot streaming disconnect-viewer

# Different action: requests host session/game termination.
marquee streaming --socket /absolute/path/wolf.sock stop SESSION_ID
```

**Disconnect viewer** stops only the receiver child owned by this Marquee viewer
supervisor. It never calls Wolf's stop API or uses a persisted PID/process-name
search as a kill target. The acknowledgement follows observed receiver exit;
`game_stop_requested: false` means Marquee did not request a game stop, not a
promise that every possible host policy retains games. **End game** remains the
separate existing host stop operation and needs session reconciliation afterward.

`viewer-status` reports receiver ownership, app ID and process liveness.
`connection_verified: false` is deliberate: an open process can be showing a
pairing/error dialog. Only host/receiver observations establish a working stream.
An absent owner reports `running: false`; disconnect then returns a descriptive
`VIEWER_NOT_RUNNING` error. A stale/unresponsive control socket is an error,
not a successful disconnect or a streaming-readiness claim.

The owner validates the selected app against the explicitly supplied Wolf socket.
The reference CLI selects by case-insensitive app title; Marquee rejects duplicate
names, control characters and option-like titles rather than silently selecting
another app. It launches loopback only, at the tested 720p/60 H.264 baseline, in
windowed mode, with system-key capture disabled, automatic host-app quitting
disabled and background gamepad input disabled. This is not a general remote-host
connection profile or physical controller acceptance.

## Private state and process ownership

- Receiver settings live in `streaming/moonlight/preferences/{config,data,cache}`
  under the selected data directory. They are separate from normal Moonlight and
  the host identity. First-time pairing must use these same directories; native
  `streaming pair` still accepts the PIN through the protected host adapter.
- The existing ST2 pilot's isolated reference settings were explicitly copied
  into this new private profile for live acceptance, preserving its pairing.
  There is no implicit migration of a user's normal Moonlight configuration.
- The foreground CLI owns a single receiver process group and an exclusive lease.
  A private `streaming/moonlight/viewer/control.sock` accepts only bounded status
  and disconnect messages. Directory permissions are 700 and socket/log 600.
- Reject unexpected control-path files/symlinks, active sockets and linked logs.
  Only a refused stale socket is replaced after acquiring ownership. No old PID
  file grants permission to signal a process. Natural receiver exit clears the
  control socket and releases the lease.
- Stop first sends TERM to the owned group, then uses bounded KILL fallback if
  needed. Guard cleanup handles errors. On Linux a parent-death signal also
  terminates the reference child if its owning thread/process disappears.
- `last-viewer.log` is private, replaced for each attempt and monitored against a
  16 MiB threshold; crossing it stops only the viewer. Do not include logs or
  copied pairing preferences in exports without a separate secrets review.

For first-time manual reference pairing, launch the installed `AppRun pair
127.0.0.1` with `QT_QPA_PLATFORM=xcb` and the three XDG variables pointing to the
private preferences directories above. Do not override HOME. Finish host PIN
acceptance through Marquee and close the pairing window before opening a viewer.
The Streaming screen now owns this pairing launch and PIN submission flow;
no unattended pairing is implied.

## Focus behavior and acceptance

The pinned receiver's input implementation releases held keys on focus loss and
supports these standard local shortcuts:

| Action | Reference shortcut |
| --- | --- |
| Switch desktop focus | Alt+Tab, with system-key capture disabled |
| Release/regrab input | Ctrl+Alt+Shift+Z |
| Disconnect receiver | Ctrl+Alt+Shift+Q, with host-app quitting disabled |

Source review alone is insufficient. The owner confirmed the Alt+Tab test on
Fedora: switch away while holding a movement direction, release it outside the
receiver, return, and game controls are not stuck. The earlier “just played”
attempt was not counted. Mouse release and ordinary shortcut/window disconnect
were tested next: the owner confirmed Ctrl+Alt+Shift+Z mouse release and
Ctrl+Alt+Shift+Q closing the receiver. The receiver returned 0 while the exact
Sonic process remained running. The operating system's window-close button is
not covered by that shortcut test. Return-to-Marquee focus automation and
controller forwarding remain separate; the owner currently has no controller.

Native command acceptance also passed: `disconnect-viewer` closed the receiver,
its owning CLI returned 0, status became inactive, and the same Sonic PID survived.
A controlled owner SIGKILL test triggered receiver parent-death cleanup; a new
owner recovered the stale socket and resumed the same game. No external process
was selected through a PID file or name search. The final explicit host stop
cleared the pilot session, and its host container was stopped cleanly.

Tests exercise real Unix sockets and subprocesses: one owner, unrelated-process
survival, malformed control requests, invalid acknowledgements, natural exit,
stale-socket recovery, unexpected control files and symlinked-log protection.
The full repository checks cover the shared implementation. Live evidence stays
under the ignored streaming pilot directories.
