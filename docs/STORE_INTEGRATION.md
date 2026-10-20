# Direct store integration: implementation handoff

Decision date: 2026-10-21. Priority: **Steam → Epic through Legendary → GOG through gogdl**, with the minimum managed compatibility foundation developed alongside Epic. [ROADMAP.md](ROADMAP.md) is the authoritative work order.

## Intended user experience

The user wants a single place for games and does not want to configure Lutris. Marquee replaces that role. A supported normal workflow is:

1. Open Marquee; existing supported installations are detected.
2. Connect the desired store and complete the store's required sign-in/MFA.
3. See owned games, including titles not yet installed, with distinct cached/offline states.
4. Select a title and installation destination; use sensible native/Windows, language, and runtime defaults with an override where meaningful.
5. Install with real progress or an explicitly labeled client handoff.
6. Play; return to Marquee; retain preferences, installation state, and verified history across restart.

A manual executable picker, Wine-prefix editor, Lutris database path, or separate launcher setup wizard is not a prerequisite. Store-mandated login, EULAs, Windows installer dialogs, and unsupported games can still require interaction; do not promise a universally unattended setup.

## Boundaries

| Component | Responsibility | Does not replace |
| --- | --- | --- |
| Marquee | UI, library identity, connections, jobs, dependency setup, runtime policy, prefixes, recipes, lifecycle and diagnostics | Store authentication/ownership requirements |
| Steam client | Steam account session, install/update/launch, Steam compatibility settings | Marquee's unified library |
| Legendary | Verified Epic backend operations | Windows compatibility runtime or Marquee UI |
| GOG adapter + gogdl | Verified GOG account/library and file/launch operations | Windows compatibility runtime or Marquee UI |
| umu + selected Proton build | Execute supported non-Steam Windows games | Store ownership, download/install jobs, per-game dependency orchestration |
| Native/emulator adapters | Run their supported installations | Original proprietary game/system data |
| Lutris/Heroic | Optional legacy import or exception fallback | Direct store integration or required onboarding |

The existing Lutris cache bridge remains in the tree. Its current automatic discovery is legacy behavior to move behind an explicit optional integration; do not grow it into the source architecture. A cached GOG/Epic entry obtained from Lutris is not a direct account connection. Do not remove installed games, prefixes, or credentials when de-emphasizing that bridge.

## Shared groundwork

1. **Inspect the current tree and preserve existing work.** Read PS1/PS2/schema/session changes before migration or API edits. There may be uncommitted concurrent work. Never reset it or assume the next schema is v4; v4 is already used for PS1 system roots.
2. **Add capability-based contracts.** Store adapters expose discovery, connect/status/disconnect, owned sync, installation inventory, install/update/verify/remove/launch where supported. Runtime adapters expose detection, setup, health, profile construction, launch, and repair. Unsupported capabilities are explicit.
3. **Separate persistent identities.** Store account + store product/build/edition + installation + launch profile. Retain canonical game/edition relationships without collapsing independent saves or entitlements. Use migrations with backups and regression fixtures; preserve existing disc IDs, favorites, personal titles, and sessions.
4. **Define connection states.** Unconnected, connecting, connected, refreshing, expired/reconnect, cached offline, and unavailable/error. Distinguish an empty account from a failed/private/unsupported response. Define multi-account behavior; never silently mix two users' tokens or ownership.
5. **Protect credentials.** Keep store/backend tokens in backend-owned protected storage or an OS credential store. Never send secrets in browser snapshots/events, launch diagnostics, logs, or argv where the backend offers a safer supported mechanism. Current API-key settings are not evidence that store authentication is implemented.
6. **Define persistent jobs.** Job IDs, target product/install, progress/log events, cancellation, retry/resume, errors, restart recovery, and state reconciliation. URI/client handoffs only report the evidence they can observe. Declare unsupported pause/resume/progress instead of simulating it.
7. **Manage helpers.** Detect existing suitable versions; select tested builds; record version/integrity/distribution provenance; stage installation; retain rollback. Handle the supported package/portable-build route rather than telling normal users to prepare PATH themselves.
8. **Extend the shared command/event surface.** Tauri, HTTP, and CLI use the same core operations. Add state refresh/invalidation for connections, library sync, jobs, helper readiness, and sessions. Guard stale async responses and duplicate actions.
9. **Design lifecycle evidence.** Define how each adapter observes the actual game's start/exit. Steam or store-client process lifetime and a quick launch helper are insufficient. Until tracking is proven, label imported/store-reported time and keep session claims unavailable.

Implement groundwork only to the level the next store needs; do not require a broad framework rewrite before the first installed Steam game can launch.

## S1A: installed Steam games — first implementation task

Before adding the Steam flow, move the existing Lutris bridge out of default onboarding/automatic discovery into an explicit optional legacy setting. Preserve its saved configurations and all external files; do not uninstall Lutris or alter prefixes. Verify a fresh Marquee setup has no Lutris prerequisite or default Lutris connection step.

1. Detect native/Flatpak Steam and its authoritative library configuration. Distinguish two clients if both exist. Use a maintained reader or a tested parser; inspect its version/API before adding a dependency.
2. Enumerate all registered Steam libraries and `appmanifest_<appid>.acf` files. Reconcile duplicate roots by canonical location while retaining genuinely different installations.
3. Parse names, app IDs, install directories, build/install state, and available size information. Do not treat every manifest as a fully ready game. Handle malformed manifests per file, offline mounts, partial installs, and deleted games without losing personal metadata.
4. Persist product/installation records and source provenance transactionally. Rescan preserves stable IDs and user edits. Do not merge equal titles across stores or editions.
5. Map into the existing Library/Game page, source/platform/install filters, and explicit run choices. Unknown data stays unknown. Fetch verified Steam artwork best-effort with procedural fallback; no API key or metadata success is required to play installed games.
6. Build executable/argv handoff for the owning Steam client. Steam keeps its own login and Proton settings. Validate missing client and client-selection cases; avoid guessing an arbitrary Windows `.exe` inside a Steam installation.
7. Reconcile install/uninstall changes after handoff, on focus, and through a bounded refresh/watch strategy. Keep offline source and missing-file states separate.
8. Test parsers/fixtures, stable rescans, duplicate paths, malformed data, native/Flatpak selection, and unavailable clients. Run an actual user-owned game and record launch/return evidence. Add real lifecycle tracking only with an adapter-specific proof.

**Increment gate:** import an existing installed Steam library and launch a real title from Marquee without Lutris or a Marquee API key; preserve IDs and user preferences after restart. No Steam password is collected by Marquee.

## S1B: owned Steam library and installation handoff

1. Verify the supported account/library mechanism against the current Steam documentation and the actual key/access model. The public user API, publisher-only endpoints, OpenID identity, and a Steam client's own authenticated session are different things.
2. A user-provided Steam Web API key + SteamID is a candidate authorized route; validate availability, profile/privacy behavior, and endpoint response semantics before committing the flow. Do not bundle a publisher secret or promise a private library from OpenID alone.
3. Add the least burdensome supported connection UI and document its prerequisites. Account connection is optional for the user, but owned-library listing is a real product capability—not a deferred metadata extra.
4. Sync owned products into a cached library; match installed records by app ID and account/store identity. Preserve family sharing/free-game/subscription/DLC semantics only where the source actually supplies them. Do not advertise an exhaustive license inventory from a narrower response.
5. Offer a verified Steam install/client-library handoff for uninstalled products. Steam performs the installation and updates. Show that handoff honestly; monitor supported local installation state rather than fabricate job percentages.
6. Test disconnected/expired credentials, private/inaccessible data, empty versus failed responses, offline cache, installation completion/rescan, and account changes. Local installed-game launch remains usable throughout.

**Full S1 gate:** installed and authorized owned listings coexist; Install/Play states are truthful, installation handoff is explicit, and restart/offline behavior is reliable. Record any account-access limitations rather than calling a partial list complete.

## C1 + E1: compatibility and Epic — second store milestone

1. Select a tested Legendary release/fork and record its version, distribution method, CLI/JSON/event behavior, authentication storage, and supported operations. Heroic uses a fork; do not assume every upstream/fork version is interchangeable.
2. Implement Marquee's Epic sign-in/reconnect/disconnect orchestration using the selected backend's supported browser/code flow. Keep required MFA with the user. Do not invent unsupported embedded-login interception.
3. Sync Epic ownership and existing backend installations, including product/app/build/DLC identity as supported. Use direct backend data, not Lutris's `service_games` cache.
4. Integrate owned/uninstalled titles into the same library model and clear connection/error/offline UI.
5. Detect/install a supported umu and Proton runtime. Stage downloads, verify available integrity evidence, pin versions, retain rollback, and avoid updating a runtime while in use.
6. Create a managed prefix and profile for the selected installation. Apply versioned, repeatable prerequisite/protonfix recipes and expose diagnostics. Keep save/config data safe during repair or runtime changes.
7. Connect real install/update/verify/download jobs, destination/storage checks, cancellation/resume where supported, and error recovery. Persist job and install state through restarts. Required store/game authentication remains the backend's responsibility.
8. Launch through the store/runtime composition and establish actual game lifecycle evidence. Handle unsupported anti-cheat/game compatibility explicitly per tested title.
9. Add cloud-save behavior only after conflict/overwrite and offline policies are tested; initially expose unsupported or disabled states honestly.

**Gate:** without Lutris/Heroic installed or configured, sign into Epic in the Marquee-managed flow → list an owned title → install → play a supported Windows title → return → restart and play again. Exercise interrupted install, auth expiry, offline cache, and runtime failure cases.

## G1: GOG — third store milestone

1. Inspect and pin a tested gogdl build, its authentication/token model, CLI/JSON support, installation layout, and supported operations. gogdl is intended to be driven by another application; its existence does not prove a complete owned-library API is exposed by every version.
2. Implement missing GOG library/account adapter responsibilities explicitly, using verified backend behavior. Present sign-in/reconnect/disconnect through Marquee with protected token handling.
3. Sync products/editions, installed builds, DLC, languages, and extras where supplied. Distinguish ownership records from downloadable artifacts and optional bonus files.
4. Select a supported native Linux build or Windows build and display the actual choice. Preserve separate installations/profiles if the user keeps both; do not silently replace save contexts.
5. Reuse persistent installation jobs and readiness checks. Launch native builds directly; Windows builds use C1 without a mandatory Lutris/Galaxy/Heroic installation.
6. Add supported update/verify/offline/save behavior with explicit limits and preservation of user data.
7. Test sign-in expiry, cached library, missing build, storage errors, interrupted jobs, restart, and both native/Windows paths where a tested title supports them.

**Gate:** without Lutris, Heroic, or GOG Galaxy, connect GOG → show owned titles → install a supported build → play → reconcile update/verify where supported → replay after restart/offline with truthful prerequisites.

## Common UI acceptance

- Home/Library show real account and installation data, with clear demo separation and one canonical page for explicitly linked releases.
- Sources are Steam, Epic, GOG; runtime/backend provenance is a separate field. A GOG game does not become a generic Wine or Lutris source.
- Connection and missing-component actions are available in Marquee. Tools path/command editors are advanced escape hatches.
- Jobs and errors update on desktop and browser; reopening the app recovers jobs and preserves user choices.
- Filters apply to the same concrete installation. Missing preferred builds/profiles offer an explicit choice.
- Metadata outages, missing artwork keys, and optional providers do not gate Play.
- Verify keyboard/controller interaction and layout on a real available surface; a successful build is not visual QA.

## Scope exclusions for this critical path

No Lutris onboarding milestone, optional launcher import expansion, torrent/tracker integration, universal Windows compatibility claim, automatic destructive prefix reset, or full metadata catalogue is required to ship these stores. Original ROM/BIOS/firmware and native-port extraction inputs remain user-supplied. Authenticated downloads of the user's owned store games are in scope.

The current task updates documentation and priorities only. It does not remove the legacy bridge, install helpers, collect credentials, migrate the library, or implement the direct stores.

## Sources and implementation-time verification

- [Steam Web API and OpenID overview](https://steamcommunity.com/dev), [authentication/ownership](https://partner.steamgames.com/doc/features/auth), [IPlayerService](https://partner.steamgames.com/doc/webapi/IPlayerService).
- [Legendary](https://github.com/legendary-gl/legendary), [Heroic reference architecture](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher), [gogdl](https://github.com/Heroic-Games-Launcher/heroic-gogdl), [umu](https://github.com/Open-Wine-Components/umu-launcher).

Recheck external CLI versions, capabilities, authentication mechanisms, and distribution conditions during implementation. No command spelling, account-access guarantee, or supported-game claim is accepted solely because it appears in an earlier plan.
