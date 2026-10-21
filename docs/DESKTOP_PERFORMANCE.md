# Desktop lag and GPU correction pass

2026-10-21. This pass identifies avoidable work in the renderer and corrects it. It is not a measured native-window before/after GPU benchmark.

## Findings and changes

| Finding | Correction |
| --- | --- |
| The fixed background rendered a large procedural SVG, enlarged beyond the viewport and filtered with a 56px blur. Cover preview changes cross-faded those layers. | New **Balanced** default uses themed CSS gradients without a blurred SVG layer. Full effects remain an explicit Settings choice. |
| Panels, buttons, cover labels and modal scrims repeatedly used backdrop blur, including within larger filtered layers. | Shared filter variables disable those passes in Balanced mode. All existing surfaces respect the same policy, including Tools. |
| A keyboard-focused cover pulsed its box shadow forever, so leaving focus on a card could sustain rendering work. | Removed the infinite pulse; the visible focus ring remains. |
| Every cover had a perspective transform. Pointer events synchronously measured the card and wrote four style values at the raw event rate. | Balanced uses ordinary 2D hover scaling. Full-mode tilt runs at most once per animation frame, stops on leave/disposal, and ignores inactive windows. |
| Procedural artwork used SVG fractal-noise filters for grain, including large hero scenes. | Grain is Full-only. Balanced keeps the underlying artwork and gradients. |
| Home shelves rendered every entry, sometimes duplicating the same large collection across shelves. | Each shelf initially renders at most 24 cards, with an explicit Show more action. Cover images use lazy loading and asynchronous decoding. The Library's existing 48-item paging is retained. |
| Home hero cross-fades could overlap during rapid preview changes; Library sorting/filtering animated the entire visible grid. | Balanced shortens hero transitions and removes grid reorder/entry animations. Full retains the longer effects. |
| Purple polled every 1.5 seconds, and the source-build dialog every 2 seconds, regardless of window activity. Identical payloads replaced component state. | Idle polling slows to 15 seconds, active jobs retain faster updates, hidden/unfocused windows stop polling, and focus resumes immediately. Unchanged payloads do not replace state. Slow requests cannot overlap; closing the component stops future work. Backend jobs continue independently. |
| A connected controller kept a requestAnimationFrame loop alive while the launcher was unfocused, potentially processing input intended for another window. | Controller polling stops while hidden/unfocused and resumes on activation. Disposed engines cannot reschedule queued frames. |
| Native event-listener cleanup raced asynchronous subscription setup; shell remounts did not release the old subscription. | Late subscriptions immediately unlisten after disposal; boot replaces the prior subscription and shell teardown releases it. Browser event responses are ignored after unsubscribe. |
| OS reduced-motion CSS shortened infinite animations without limiting their repeat count, and Svelte transitions only checked the app toggle. | Reduced-motion CSS limits repeat count; application transitions/tilt now combine the app preference with the OS preference, including live OS changes. |
| The clock updated reactive state while the app was inactive. | No inactive clock state updates. |

## User controls

**Settings → Look and feel → Visual effects → Balanced** is the default, including for existing profiles that have no saved effects choice. It preserves themed artwork, hover/focus indication, game covers and navigation. Full enables the larger blur, grain and tilt effects. Reduce motion is independent and also respects the operating-system preference.

This pass does not change emulator rendering settings, GPU drivers, hardware acceleration, resolution, refresh rate or power management. User data and game files are unchanged.

## Evidence and limits

The available host snapshot showed an RTX 5090 at 32% GPU utilization and 48.30 W, with approximately 1989 MiB in use. The process sample showed Firefox and GNOME compositor activity, but no Marquee/WebKit renderer. Those are whole-system observations, **not Marquee measurements** and not a baseline for an improvement percentage. The available computer-use inventory exposed no app or browser surface, so live interaction, frame-time profiling and visual comparison could not be performed here.

Regression tests verify background controller safety, stop/resume behavior, non-overlapping requests, disposal while requests are pending, idle polling cadence, OS/application motion preference combinations and subscription cleanup on remount. Existing game-library, launcher and build tests remain in the check suite.

Final `scripts/check.sh` passed: 118 Rust tests, 49 frontend tests, Clippy, native Tauri compilation, Svelte checks with zero warnings, and the production frontend build. A parallel-test `ETXTBSY` race in the earlier source-build wrapper test was corrected by invoking its temporary script through Bash; production launcher behavior was unchanged.

## Remaining native acceptance

1. Use the same native build, window size, display scaling, library, route and foreground state for each comparison. Separate development-server/HMR behavior from a production build.
2. In Balanced mode, leave a cover keyboard-focused for at least 30 seconds, then scroll the Library and sweep across Home covers. Record renderer CPU, GPU utilization/power, frame timing and dropped frames.
3. Repeat in Full mode. Switch back to Balanced, minimize/unfocus the launcher, and run a game. Confirm the launcher stays quiet, controller input does not reach it, and backend downloads/build jobs continue.
4. Check dark/light themes, desktop/Big Screen, game covers, filters, modal readability, keyboard/controller focus and shelf Show more behavior. Confirm OS reduced motion disables application transitions.
5. If lag remains, capture a native WebKit/compositor trace and attribute it to the exact process. Investigate image dimensions, remaining masks/shadows, library size, GPU/WebKit driver paths and display scaling using that trace. Do not disable hardware acceleration or alter GPU drivers based only on this code review.
