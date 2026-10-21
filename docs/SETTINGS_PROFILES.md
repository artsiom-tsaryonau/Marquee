# Managed emulator settings profiles

Implemented 2026-10-21. Roadmap Tools/emulator priority 1 starts with DuckStation.
Other emulator profiles remain separate reviewed recipes.

## User flow

Tools → DuckStation → Configure → Settings profile offers Recommended for this
machine, My settings, and Leave settings unchanged. Renderer, internal resolution
and VSync are editable in My settings. Saving requires a closed managed emulator.
The details area reports detected GPU vendor/device/driver, active display modes
when available, unknowns, current settings and which values Marquee manages.

Both library Play and Tools Open prepare the profile in Marquee's isolated
DuckStation settings folder. Library launch treats optional profile failures as
best effort, preserving Play; Configure reports errors rather than hiding them.
Tools Open reports an invalid profile before opening the tool. The existing
launch-owned setup-wizard flag is also written for Tools Open, not just Play.

CLI uses the same core as desktop and browser:

```bash
marquee emulators profile duckstation
marquee emulators profile-set duckstation --mode recommended
marquee emulators profile-set duckstation --mode custom --renderer OpenGL --scale 2 --vsync
marquee emulators profile-set duckstation --mode disabled
```

Preview is read-only. Disabled stops applying graphics preferences and leaves
current values in place; it does not reset the emulator or delete saves.

## Verified recipe v1

Keys and enum values were checked against official DuckStation source commit
`48a1028f9a2f3a3c907cca82ce41d30c9ef2d747` on 2026-10-21:

- `[GPU] Renderer`: Automatic, Vulkan, OpenGL or Software.
- `[GPU] ResolutionScale`: integer scale. Marquee offers 1/2/3/4/6/8.
- `[Display] VSync`: boolean.
- `[Main] SetupWizardIncomplete`: false before a managed launch.

Evidence: [settings loader, writer and renderer names](https://github.com/stenzek/duckstation/blob/48a1028f9a2f3a3c907cca82ce41d30c9ef2d747/src/core/settings.cpp)
and [wizard handling](https://github.com/stenzek/duckstation/blob/48a1028f9a2f3a3c907cca82ce41d30c9ef2d747/src/duckstation-qt/qthost.cpp).
No upstream implementation code is bundled by this recipe.

Linux host detection reads DRM vendor/device IDs and driver symlink names.
An installed `/usr/bin/xrandr --query` can provide current resolution/refresh;
probes have a timeout, output bound and no shell invocation. Supported but
inactive modes are ignored. Xwayland output is labeled and is not treated as
physical-monitor evidence. Missing tools, GPUs or modes remain unknown.

Recommended always delegates renderer selection to DuckStation's Automatic.
For a detected hardware driver on X11, it chooses 2× at display heights of at
least 720 and 3× at 1440 or above; otherwise it stays at 1×. VSync is enabled
only for reported X11 refresh rates between 50 and 360 Hz. This is a conservative
policy, not a benchmark, compatibility rating or guaranteed performance target.
Native Wayland monitor enumeration and hardware-specific tuning remain follow-up
work; unknown hardware does not prevent launch.

## Preservation contract

`duckstation/marquee-profile.json` beside the isolated `settings.ini` records
recipe version, selected mode/custom options, last applied values, prior values
for each changed key and released ownership. It does not change library schema,
identity, history, firmware, controller bindings, saves or memory cards.

Initial application fills absent keys only. Preexisting values are preserved.
Later applications can update a managed key only while its current value still
matches the receipt. An external edit, deletion or duplicate key releases that
key permanently; later coincidental equality does not reacquire it. The dialog
reports the preserved value rather than pretending the requested value applied.
Unsupported versions, malformed receipts, oversized files and symlink redirects
are refused. Other settings and comments are retained. Writes use atomic files;
the receipt is published first so interruption conservatively releases ownership
rather than authorizing later overwrites. A library session or detected managed
DuckStation process blocks explicit profile editing.

## Verification and remaining live gates

Tests cover preset fallback, active-mode parsing, idempotence, edits/deletions,
duplicate keys, disabled mode, unsafe files, wizard suppression, API transport,
persistence through core restart and preservation of library contents. The full
repository check includes Rust formatting/clippy/tests, native Tauri compilation,
frontend diagnostics/tests and the production build. Earlier Rust formatting
failures in lib.rs/metadata.rs/wanted.rs were normalized without behavior changes.

No visual or real-game acceptance is claimed. Verify actual renderer startup,
VSync/presentation and controller/save behavior on supported hardware. The native
window and browser were unavailable during implementation; no personal emulator
settings or user library were modified to simulate that gate.
