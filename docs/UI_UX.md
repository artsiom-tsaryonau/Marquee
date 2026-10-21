# UI and UX implementation pass

Updated 2026-10-21 after the screenshot/source audit. This is an implementation
record, not a replacement for [the roadmap](ROADMAP.md). Direct Steam, then
compatibility/Epic, then GOG remain the store milestones.

## Implemented

- Home excludes catalogue-only discoveries, groups explicitly linked game
  identities in shelves/counts, limits platform shortcuts to four, and keeps
  the desktop hero stable. Big Screen focus can still preview a game. Mouse
  hover no longer changes the main Play target.
- Library separates Your library from Catalogue discovery. Catalogue imports
  stay available in discovery. Filters, search, grouping and return position
  remain shared, with distinct catalogue/ownership language.
- Global Add games opens the Library system picker instead of assuming PS2.
  Existing folder imports remain the supported flow; this pass does not add
  store connections or an archive-import wizard.
- Cards show readable titles underneath, with calmer generated art. Offline
  drives, missing files, missing emulators, missing firmware, wishlists,
  catalogue records and uninstalled entries keep distinct status text.
  Landscape shelves contain portrait covers rather than cropping them across
  the full landscape frame. Shelf arrows and thin scrollbars expose scrolling.
- Game pages use a consistent title, separate release details, labeled primary
  actions and optional identification/metadata details. Portrait art becomes a
  soft background with a foreground cover at larger widths. Title editing has
  visible Save/Cancel controls under Manage game. Favorites expose pressed
  state. Descriptions use their supplied source instead of a fixed credit.
- Recommendations require matching genre and a launchable installation,
  exclude the current canonical game, group duplicate identities, and are
  labeled More in this genre. Sharing an emulator/source is not similarity.
- Hardcoded ProtonDB and anti-cheat claims have been removed. Launch overlays
  name the selected emulator. Optional metadata/firmware verification still
  does not block a supported Play action.
- Tools retains platform-first filtering and shared cards, with secondary type,
  availability and attention filters plus a compact view. Emulator installation
  is labeled Installed, not Ready. Required firmware gaps are explained.
  Firmware filenames expand on demand instead of setting every card's height.
- Ports shows setup requirements, supported actions and installation details
  instead of a decorative feature banner. Missing-ROM/data actions open the
  existing folder fallback rather than reinstalling the engine. Requirement
  copy describes current behavior rather than future development jobs.
- Settings has Appearance, Games and controls, Library locations, Artwork/API
  keys and Device sharing categories. An optional persistent expanded sidebar,
  distinct Tools icon, current-page semantics and larger Big Screen controls
  improve navigation. DuckStation setup routes through Tools.
- Launcher, source, emulator and installer dialogs isolate background controls,
  contain Tab and spatial focus, consume Escape once and restore prior focus.
  Held controller directions repeat after a delay; A/B remain edge-triggered.
  The unimplemented controller Menu hint has been removed.
- Exact catalogue matching is offered only for available `ps1_iso`/`ps2_iso`
  media, matching the backend capability. Other formats can use manual linking
  through Catalogue discovery. Checksums remain optional and never gate Play.

## Shared Tools catalogue follow-up

Roadmap Tools priority 2 brings native ports into the same ToolCard grid as
managed PCSX2/other emulators and launchers. Platform filtering stays primary;
Native ports is an optional type filter. Search includes game and port names.
Configure opens a contained modal with requirements, installation/version,
engine-versus-data status, appropriate install/update/open/folder actions,
progress and visible retryable errors. The old PCSX2-specific Tools header
installer is removed in favor of shared catalogue actions. Ports discovery and
Library game entries are preserved. Shared action dispatch prevents missing-data
states from accidentally reinstalling an engine. No new data-validation or
verified gameplay capability is implied.

## Port readiness contract

The current Rust port service reports `ready` when the engine is installed.
It does not validate the required retail data. The desktop projection therefore
maps this to `installed`: **Installed · game files unchecked**, with **Open
port** as the action. An available engine update does not establish game-data
readiness either. Opening a port may enter its own setup window.

The UI's `ready` state remains available for explicitly ready entries (including
existing demo fixtures). Do not map real engine installation to Ready to play
until a port-specific data check is implemented and verified. Refresh status
refreshes installation/update information; it is not a content validator.

## Verification

Frontend `pnpm check`, `pnpm test` and `pnpm build` cover compilation,
availability/actions, exact-matcher capability, personal hero selection,
unchecked installed ports, modal isolation/restoration and held-direction
repeat alongside existing regressions. Targeted Rust `ports::` tests cover the
catalogue and installer preservation/integrity behavior.

The subsequent DuckStation profile step normalized the existing Rust formatting
differences and passed the full repository checks, including native compilation.

No browser/native UI surface was available for this pass. Builds and mocked DOM
regressions are not rendered-layout or native-gamepad acceptance. Still inspect:

- Desktop and Big Screen at 1280×720, 1920×1080 and wide desktop sizes; dark and
  light themes; reduced motion; long titles; narrow layouts and text scaling.
- Actual contrast, card/status wrapping, collapsed/expanded sidebar behavior,
  portrait-only heroes, empty scopes and large-library scrolling.
- Keyboard/controller focus through each dialog, cancellation, game launch and
  return; native Tauri input remains a separate acceptance gate.
- Real port engine setup/game-data readiness and emulator controller/save gates.

## Follow-up boundaries

Persistent Downloads/jobs, direct store connections, firmware/media acquisition,
port data discovery/extraction, save management and normalized identity/defaults
remain backend/workflow milestones. This pass does not simulate their completion.
Persisted scan history, richer recommendations and provider-specific metadata
editing are further improvements; missing files still use existing library
management rather than a new relocation wizard.
