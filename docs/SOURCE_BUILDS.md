# Managed emulator source builds

Implemented 2026-10-21: an opt-in mGBA pilot on Linux x86_64. Existing checksum-verified GitHub binaries remain the default for the other seven managed emulators. This does not build every emulator or enable new library platforms automatically.

## Use

1. Have Git and working **rootless Podman** available. Marquee checks both and refuses a rootful builder. It does not run sudo or install host development packages. Packaging those prerequisites remains follow-up work.
2. Open **Tools → Emulators → mGBA → Build from source**. The job runs in the backend; closing the dialog or refreshing the browser does not cancel it.
3. Read the phase and expandable log. Cancel waits for the active subprocess to stop. Failed, cancelled and interrupted jobs can be retried from a fresh staging directory. A process restart reports an interrupted job; it does not silently resume compilation.
4. Choose a specific game folder in the dialog, then Open. That folder is mounted read-only; settings and saves use Marquee's writable directories. Home itself and `/` are refused as game folders. The wrapper consumes the saved `--marquee-games=<folder>` argument before invoking mGBA.
5. Qt is the normal Open frontend. Advanced game-only launch uses separate argument fields: `--marquee-sdl`, `-f`, `/absolute/path/to/game.gba`. A supplied game file is mounted individually, including when it is outside the chosen folder. There is no shell expansion. This standalone launch is not a Marquee library session and does not add playtime.
6. Rebuild keeps one previous program and receipt. **Restore previous version** swaps programs without touching saves. Remove keeps saves unless “remove with saves” is explicitly chosen.

CLI (with no other Marquee instance using that data directory):

```sh
marquee emulators build mgba
marquee emulators build-status mgba
marquee emulators rollback mgba
```

The build command waits and reports success/failure. Browser and Tauri share `build_emulator`, `emulator_build_status`, `cancel_emulator_build` and `rollback_emulator`. Status responses contain only the last 32 KiB of the local log. Demo mode never claims to compile or install anything.

## Recipe, validation and provenance

- Reviewed source: [mGBA 0.10.5](https://github.com/mgba-emu/mgba/tree/26b7884bc25a5933960f3cdcd98bac1ae14d42e2), commit `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`. Fetch requests that exact commit, checkout verifies HEAD, and unexpected submodules fail closed. This revision has no submodules; generic recursive-submodule support is not implemented.
- [Containerfile](../catalog/source-builds/mgba/Containerfile): Debian base pinned by SHA-256, dependencies installed inside rootless Podman, Qt and SDL built with CMake/Ninja and two compiler workers. FFmpeg, Lua, Discord RPC, editline and GDB stub are disabled. This is a reviewed recipe, not a generic arbitrary-repository executor.
- Package repositories are **not snapshot-pinned**. The image contains `/opt/mgba/packages.txt` with exact installed package versions. Source/base pins establish identity; they are not a signature or a claim of bit-for-bit reproducibility.
- Both compiled frontends must pass `--version` before promotion. `current/source-build.json` records the commit, immutable local image ID, recipe and launcher hashes, and validation boundary.
- Build subprocesses use argv, null stdin, their own process group, per-stage deadlines (up to two hours for compilation), cancellation and a 16 MiB log limit. Network is available during source/dependency acquisition; emulator runtime uses `--network=none` and `--pull=never`.
- Promotion retains `previous` and restores `current` if receipt persistence fails. This handles reported filesystem/DB errors; it is not a journaled transaction across a power failure between renames. Inspect receipts and restore the previous directory if power loss interrupts promotion.

## Storage and runtime boundary

| Location | Contents |
| --- | --- |
| `<data>/builds/mgba/` | Last status, bounded build log and Podman preflight information |
| `<data>/runners/mgba/current/` | Launcher, image identity, installed receipt and source provenance |
| `<data>/runners/mgba/previous/` | One previous program/receipt for rollback |
| `<data>/mgba/xdg-config/` | mGBA settings |
| `<data>/mgba/xdg-data/saves/`, `states/` | Explicit save and state locations |
| Rootless Podman image storage | Compiled binary, runtime libraries, compiler/dependency layers and package manifest |

The compiled program runs in its immutable local OCI image rather than as a host-linked executable. Podman remains a runtime prerequisite. Open detects missing/pruned images and asks for a rebuild. Image layers are a shared Podman cache: removing a Marquee program does **not** automatically delete them or images used by another data directory. Use Podman's own storage tools to inspect/remove unused images. Abrupt process death can leave temporary `.source-*` staging directories and builder cache; these are not resumable jobs.

Interactive runs expose the selected game files, managed data directories, applicable graphics device, and the specific display/audio sockets and X11 authentication needed to render on the desktop. They do not mount the whole home or session-runtime directory. `--version` probes expose **no host mounts, desktop credentials, devices or session sockets**. Runtime controller passthrough, GPU/vendor differences, audio, Wayland/X11 rendering and graceful game shutdown still need live acceptance. Opening a container frontend does not embed its render output inside Tauri.

## Verification and next steps

On Fedora 44 with rootless Podman, the actual pinned recipe compiled both frontends. The Marquee CLI completed fetch → build → validation → promotion using `/tmp/marquee-source-build-verification`, separate from the real library. A second build retained the previous program, and CLI rollback succeeded. Qt and SDL version checks, including the mount-free launcher branches, succeeded. The full repository check passed (118 Rust tests, 42 frontend tests, Clippy, Tauri compile, Svelte checks and production build). No ROM, BIOS or real library file was changed. No gameplay or interactive desktop rendering was verified.

Next gates:

1. Live mGBA game/audio/controller/save/exit acceptance with a user-owned game; implement device access only for the selected controller hardware.
2. GB/GBC/GBA system descriptors, media identification/import, mGBA runner, per-system choices, session tracking and save acceptance. Until then Tools remains explicitly standalone.
3. Managed prerequisite delivery, smaller runtime-only images, storage accounting/cleanup and crash-safe promotion recovery.
4. Pin a Dolphin revision and its recursive submodules, select Qt/batch and optional NoGUI frontends, measure build/storage cost, then add a separate reviewed recipe. Dolphin is **evaluated as the next candidate, not implemented or installed**. Its larger dependencies and distinct no-GUI platform support need independent acceptance. See [presentation research](EMULATOR_PRESENTATION.md).
5. Add further source recipes only where binary acquisition is unavailable or a concrete patch/build option justifies maintenance. Keep source recipes separate from game-library adapters.
