# Desktop rendering, performance and production launch

Implemented and live-tested on Fedora 44 / NVIDIA RTX 5090 / Wayland / WebKitGTK 2.54.1, 2026-10-21. This replaces the earlier lag-pass, live-profile and rendering notes.

## What was wrong

A live profile of the dev build (`target/debug`, Vite) showed WebKit on its **non-composited software painting path**. The renderer main thread averaged 24% of one core and peaked at ~100% while drawing SVG gradients and CSS drop-shadow blur; the native Rust and network processes were nearly idle, and GPU use was 0–4%. Stack snapshots showed `NonCompositedFrameRenderer`, Skia gradient fills and `FEDropShadowSkiaApplier`.

The cause: `scripts/run.sh` set `WEBKIT_DISABLE_DMABUF_RENDERER=1` for every launch, to avoid the NVIDIA/Wayland "Error 71" startup failure. In WebKitGTK 2.54.1 a nonzero value returns before even the shared-memory transport is added, so `canUseHardwareAcceleration()` is false and accelerated compositing is off ([`AcceleratedBackingStore.cpp`](https://github.com/WebKit/WebKit/blob/webkitgtk-2.54.1/Source/WebKit/UIProcess/gtk/AcceleratedBackingStore.cpp), [WebKit graphics docs](https://docs.webkit.org/Ports/WebKitGTK%20and%20WPE%20WebKit/Graphics.html)). Painting then runs on the main thread and delays scrolling and hover.

The profile is a development-session capture, not a frame-time benchmark; no FPS or speedup is claimed from it.

## Normal launch

```sh
./scripts/run.sh desktop     # optimized standalone app; no Vite or web server
./scripts/run.sh both        # optimized app plus the authenticated local web surface
./scripts/run.sh build       # build production without opening a window
./scripts/run.sh dev         # explicit browser development server with hot reload
```

The desktop/both modes use `target/release/marquee-desktop`, resolving custom Cargo target directories through `cargo metadata`. They build on first launch and when source files or source-directory mtimes are newer than the executable, excluding dependency/build-output directories. Directory mtimes also detect deleted source inputs. `MARQUEE_REBUILD=1` forces rebuilding. Build errors stop launch; a failed build never silently opens an older executable. `pnpm run:desktop` and `pnpm build:desktop` are equivalent entry points.

## Rendering policy

The native Linux entry point selects rendering settings **before GTK/WebKit initializes**. The launch script no longer unconditionally disables DMA-BUF. This also covers direct execution of the built binary.

| `MARQUEE_RENDERER` | Behavior |
| --- | --- |
| unset / `auto` | NVIDIA kernel driver + Wayland session: shared-memory transport with accelerated compositing eligible. Other sessions: normal GPU transport. |
| `shared-memory` | `WEBKIT_DISABLE_DMABUF_RENDERER=0`, `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1`. Keeps the accelerated compositor while avoiding direct hardware-buffer transport. |
| `gpu` | Enables the normal DMA-BUF path without forcing shared memory. Still reproduces Error 71 on the tested NVIDIA/Wayland host. |
| `software` | Compatibility fallback: `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Disables accelerated compositing on WebKitGTK 2.54.1; animations/scrolling can be expensive. |
| `x11` | Selects GDK's X11 backend and normal transport. Requires an X server/XWayland. Not live-validated. |

```sh
MARQUEE_RENDERER=shared-memory ./scripts/run.sh desktop
MARQUEE_RENDERER=software ./scripts/run.sh desktop
MARQUEE_RENDERER=x11 ./scripts/run.sh desktop
```

Explicit Marquee modes override inherited DMA-BUF flags. With `MARQUEE_RENDERER` absent, deliberate legacy overrides are preserved: nonzero `WEBKIT_DISABLE_DMABUF_RENDERER` keeps software mode, an explicit zero chooses normal GPU transport, and `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1` chooses shared memory. Use `MARQUEE_RENDERER=auto` to override inherited flags. Unknown mode names fail with a clear error.

Renderer changes need a **new process**; a second launch focuses the existing single-instance window and cannot change its environment. Automatic selection is a heuristic, not a GPU benchmark or crash-retry loop; mixed-GPU systems or other drivers may need an explicit mode. The app prints its selected mode at startup and does not change drivers, kernel settings, power policy or system environment.

## Visual effects: Balanced (default) and Full

**Settings → Look and feel → Visual effects.** Balanced applies to existing profiles with no saved choice; Full re-enables the heavier effects. Reduce motion is independent and also follows the OS preference, including live changes.

Balanced changes from the lag pass:

| Area | Change |
| --- | --- |
| Background | Themed CSS gradients instead of an enlarged, 56px-blurred SVG; no cross-fading blurred layers |
| Blur / shadows | Backdrop blur disabled; game-logo, tool-icon, dialog-icon and favorite-badge drop-shadows Full-only; muted tiles skip saturation filter |
| Covers | Ordinary 2D hover scale; Full-mode tilt limited to once per animation frame and stops on leave/disposal/inactive window; infinite focus pulse removed |
| Artwork / grain | SVG fractal-noise grain is Full-only |
| Home / Library | Shelves render at most 24 cards with "Show more"; lazy, async-decoded covers; Library keeps 48-item paging; shorter hero transitions; no grid reorder animations |
| Polling | Purple 15 s idle, source-build dialog likewise; faster while a job is active; stops while hidden/unfocused; unchanged payloads don't replace state; requests cannot overlap |
| Controller / events | Controller polling stops when unfocused and resumes on activation; late/duplicate native event subscriptions are released; inactive clock doesn't update state |

Real cover art, themed backgrounds, selection indication and navigation are retained. Further raster-caching of procedural SVG artwork is an optional follow-up; not all SVG painting has been eliminated. Emulator rendering, GPU drivers, resolution and power management are untouched.

## Verification

`python3 scripts/probe-rendering.py` is an optional synthetic benchmark (needs `gjs`, GTK3, WebKit2 4.1 typelib, a live desktop session). It opens temporary non-focusable windows with ephemeral profiles and a fixed 1440×900 SVG/shadow scene, runs each mode for eight seconds and prints JSON.

| Transport | Outcome | Median interval | p95 | Frames over 33.34 ms |
| --- | --- | ---: | ---: | ---: |
| Direct GPU | Wayland Error 71; exited | — | — | — |
| Shared memory, acceleration enabled | Compositor + scrolling threads, NVIDIA/DRM descriptors | 16 ms | 17 ms | 0 / 437 |
| Software fallback | No compositor/scrolling threads or graphics descriptors | 64 ms | 76 ms | 110 / 110 |

These are `requestAnimationFrame` intervals for a synthetic scene, not input latency or a promised FPS in a large library.

The optimized executable, started with automatic mode and a disposable profile, selected `shared-memory`, had the compositor, scrolling threads and NVIDIA/DRM access, and showed no Error 71. The production executable with the real library also selected shared memory with no Error 71.

Last full check: 121 Rust tests, 44 frontend tests, four launch integration tests, Clippy, native compilation, zero Svelte warnings, production build.

## Remaining acceptance

The user reported that the production app felt much faster, but the whole window
flashed when switching tabs or opening pages. The route wrapper had a delayed
incoming fly/fade and an outgoing fade, blending two full pages while exposing
the backdrop. Navigation now replaces the keyed page immediately with no
whole-page opacity/transform animation. Local artwork and dialog effects remain.
This addresses a likely contributor; visual confirmation of the remaining flash
is still required. Shared-memory accelerated rendering stays enabled.

1. Open the real collection in the production app and repeat scrolling, cover preview, Tools and game-return behavior; record renderer CPU, GPU use and frame timing with the same build, window size and scaling for each comparison.
2. Leave a cover keyboard-focused for 30 s in Balanced, then scroll the Library and sweep Home covers; repeat in Full.
3. Minimise or unfocus the launcher and run a game: it should stay quiet, controller input should not reach it, and backend downloads/builds should continue.
4. Check dark/light themes, desktop/Big Screen, modal readability, keyboard/controller focus and that OS reduced motion disables transitions.
5. If lag remains, capture a native WebKit/compositor trace to attribute it before changing effects or GPU settings.
