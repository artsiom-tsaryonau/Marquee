# Tools and launcher UI

Current shortcut features were implemented 2026-10-20. Direction updated 2026-10-21: [direct store integration](STORE_INTEGRATION.md) is the main workflow; Lutris/Heroic shortcuts are optional legacy tools. Opening their windows is not store-account integration.

The target normal flow detects and manages Steam/Legendary/gogdl/umu/Proton dependencies from Marquee. Users should not need this executable/argument editor to connect stores or play a supported game. The editor remains an advanced escape hatch. Open **Tools** from the sidebar or Library header. Recognisable store/launcher marks are catalogued in [launcher branding](LAUNCHER_BRANDING.md).

## What works

- Launcher cards show readiness, executable, configured argument count, and pin state.
- Steam, Lutris, and Heroic command presets are checked against the host's PATH. AppImages, Flatpak commands, native games, and other applications can be configured manually.
- **Add launcher → Save launcher** persists a shortcut. It does not execute anything. **Open** starts the saved command with an argument array and optional working directory.
- **Configure** edits an existing shortcut. Each argument has its own field, preserving spaces and shell-looking text literally. The desktop executable picker selects a file; the browser accepts a path on the machine running Marquee.
- **Pin** sorts launchers first. **Remove** forgets a custom configuration without touching its application files. **Reset** clears an override of a built-in preset, allowing default detection again.
- Missing/non-executable programs and unavailable working directories block Open. Readiness checks the executable and directory, not whether a launcher is authenticated or whether arbitrary arguments are meaningful to it.
- Launcher changes use the shared core API and `launchers_changed` event, so native and browser surfaces refresh together. An older refresh response cannot replace a newer saved configuration.
- Search and Ready-only filtering apply to the tool cards. Category visibility remains a local browser/window preference.
- PS1/PS2 default runners and setup, installed native ports, and the [seven managed emulator recipes and BIOS panel](EMULATORS.md) are available on this screen. Standalone emulator opening is distinguished from game-library launch support.
- The library has a collection header, relevant source chips, a clear-filters action, and explicit empty-filter states. It remembers the selected installation per canonical game in local preferences; the preferred installation is only used if it matches the current filters.
- Keyboard/controller focus skips disabled controls. Back closes the launcher editor before navigating away.

## Managed Purple for Aion 2

Tools now includes a dedicated **Purple for Aion 2** card. **Install Purple** creates an isolated umu/GE-Proton environment and applies the referenced dependency recipe, then opens the official installer. **Open Purple for Aion 2** opens that managed launcher. No Lutris or manual shortcut configuration is required. Setup progress, stop/retry, runtime/prefix/log details are shared with the browser surface. This is separate from generic shortcuts. Aion 2 Linux compatibility and live launcher setup remain unverified; see [the recipe and acceptance gate](PURPLE.md).

## Advanced shortcut setup (implemented)

1. Open **Tools** and find a detected launcher, or choose **Add launcher**.
2. Set its name and executable. A bare command uses PATH; a file path must be absolute. `~` and shell environment expansion are not performed.
3. Add any arguments individually. For a Flatpak installation, choose its Flatpak executable, then provide the required run arguments and application ID as separate values.
4. Supply an absolute working directory if the application requires one. Save, check its status, then choose **Open**.
5. Pin frequently used launchers. Use **Refresh launchers** after installing an executable or making it executable outside Marquee.

For an existing OpenGOAL installation, add its installed launcher or runtime with the working directory and arguments appropriate to that installation. This does not download game data or perform extraction.

## Storage and API

Configurations are saved in the existing SQLite `settings` table under `external_launchers.v1`. No database schema migration is needed. IDs remain stable during edits. Configuration validation limits names, executable paths, arguments, and entry counts before saving. Launcher processes are reaped after exit.

Shared commands: `list_launchers`, `save_launcher` (`launcher` object), `remove_launcher` (`id`), and `open_launcher` (`id`). Saving/removing emits `launchers_changed` after persistence. Arguments are passed directly through Rust's `Command`; there is no implicit shell.

The web surface uses the same backend commands as the desktop shell; there is no mock mode.

## Scope and verification

Opening a generic launcher shortcut does not import its games, discover accounts, install Windows compatibility environments, or track game processes started from its window. The [legacy Lutris bridge](#legacy-lutris-library-bridge) below reads existing installed games and cached store listings; it is not the direct Steam/Epic/GOG strategy and is not a required onboarding step. Native game shortcuts also use the launcher lifecycle, without library/session tracking. Managed OpenGOAL extraction and the normalized game/edition/installation backend remain separate milestones.

Checks passed: 59 Rust tests, 28 frontend tests, Svelte check with zero errors/warnings, desktop Rust check, production frontend build, and whitespace validation. Regression tests cover literal arguments and working directory, persistence/reopen, validation, API events, stale refresh ordering, edition preference filtering, and disabled navigation.

No browser or native-app surface is available to this tool session. Rendered layouts and actual Steam/Lutris/Heroic windows therefore remain unverified. Launcher execution was tested using a temporary local Python capture program, without opening the user's installed launchers.

## Legacy Lutris library bridge

**Priority superseded.** Marquee is meant to replace the separate library-manager role and does not need existing Lutris configurations imported. The direction is [direct Steam → Epic/Legendary → GOG/gogdl](ROADMAP.md) with [Marquee-managed compatibility](STORE_INTEGRATION.md). This bridge is optional legacy functionality from the earlier Lutris-first approach: do not expand it into a required source, installer, login flow or Wine-management dependency. The bridge is now disconnected by default and shown through Tools → Legacy integrations. Existing explicitly saved connections remain active.

**Implemented**

- On startup Marquee may suggest available native/Flatpak database paths, but a fresh setup does not open them or import games. Only an explicitly saved enabled connection reads the library. Existing saved connections/disconnections win over defaults. Connection settings allow another path, backend command and installed-only mode. One database is handled at a time (native preferred; prefixes are never mixed).
- Configured installations and cached GOG/Epic/Steam listings enter the unified library with their own store identity and Lutris launch provenance. Cached listings stay uninstalled with unknown platform unless verified. Matching uses **service plus product ID**, never title alone; database paths feed entry IDs so databases cannot collide. A cached entry becoming a configured installation changes its projection ID; durable provider-to-installation mapping is follow-up work.
- Lutris playtime is converted from hours to minutes; the launcher handoff is not credited as gameplay.
- Installed games hand off via `lutris:rungameid/<id>`; supported cached entries via Lutris's service-install URI (payload as one path segment). This opens Lutris's own flow; it does not guarantee an unattended install or bypass store authentication.
- Read-only: selected `games` and `service_games` fields only; no tokens, cookies, Wine YAML or `details` payloads; the source database is never written.
- Refreshes on focus and every minute while visible; this reads the local cache and starts no online sync. The library renders 48 cards at a time with Show more. Disconnect removes only the imported view.

Sources checked 2026-10-21: [Lutris](https://github.com/lutris/lutris), [FAQ](https://lutris.net/faq), [database schema](https://github.com/lutris/lutris/blob/master/lutris/database/schema.py), [GOG service](https://github.com/lutris/lutris/blob/master/lutris/services/gog.py), [Epic service](https://github.com/lutris/lutris/blob/master/lutris/services/egs.py), [CLI handling](https://github.com/lutris/lutris/blob/master/lutris/gui/application.py), [URI parsing](https://github.com/lutris/lutris/blob/master/lutris/api.py). Lutris's public catalogue is not a download entitlement, and store caches can be stale.

**Remaining legacy-only work:** keep regression coverage; defer account sync, new installer handoffs and migration features. The opt-in/default-onboarding change is implemented.

**Verification.** Seven adapter regression tests pass (native/Flatpak defaults, store-aware matching, installed-only/disconnect, nonmutating reads, encoded handoff URIs, schema/missing-file). On this machine the probe found 4 installed entries and 1,164 cached listings (651 Epic, 215 GOG, 298 Steam), 1,167 entries after matching, and the source database checksum was unchanged. No real game, installer or login was run.
