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
  store connections. A subsequent roadmap step adds the [archive importer](INTAKE.md).
- Cards show readable titles underneath, with quiet platform-mark/title fallbacks, including failed cover loads. Offline
  drives, missing files, missing emulators, missing firmware, wishlists,
  catalogue records and uninstalled entries keep distinct status text.
  Continue playing uses equal-height artwork with variable widths measured from
  the image, preserving portrait, square and landscape proportions. Main grids
  keep equal invisible slots with bottom-aligned, uncropped covers. Real artwork
  has no colored side panels or resting border/shadow; hover follows its bounds
  and keyboard/controller focus has a distinct outline. Shelf arrows and thin
  scrollbars expose scrolling.
- Game pages use a consistent title, separate release details, labeled primary
  actions and optional identification/metadata details. Portrait art becomes a
  soft background with a foreground cover at larger widths. Home confines its
  darker cover backdrop to the hero so it fades before the shelves. Title editing has
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

## Mixed artwork follow-up

Continue playing prefers dedicated landscape art when available, otherwise the
original cover. Image dimensions determine card proportions after loading; an
estimated platform case ratio reserves space before that. Exceptionally narrow
or wide images have bounded shelf widths and remain contained without cropping.
Captions reserve two title lines. Missing artwork shows a muted surface with
platform identity and a readable title instead of repeated geometric pictures.
The hero foreground poster also contains the full cover.

Svelte checks, frontend regression tests and production compilation validate the
change; rendered layout, image-load movement, hover/focus and native gamepad
acceptance at the sizes above still require a live UI surface.

## Library and Home follow-up (recorded 2026-10-21)

The former root UX checklist recorded these implemented refinements:

- Source dropdown and shared `.ui-select`; run-method filtering removed from the
  library while edition/run selection stays on the game page. Player mode uses
  single/multiplayer/co-op; co-op shares multiplayer until richer metadata exists.
- Grouped platform tags, catalogue source links in metadata, cover attribution,
  and “Not in library” discovery cards with no Play action.
- One Add games menu; labeled platform chips ordered by collection size; stores
  grouped as Other. Search/Installed/Favourites/Filters/Sort form the primary row,
  with secondary filters on demand and only applicable sorts shown.
- Search/filter/sort, loaded cards and scroll survive game-page Back navigation;
  incremental scrolling retains a load-more fallback; Esc clears search.
- Readable two-line titles, full title on hover, region in subtitles, removed
  duplicate generated-art titles and corrected zero-hour presentation.
- Compact Home hero/poster/backdrop, recent-play selection, overview/platform
  shortcuts, wishlist row and muted slate branding.

Games without files are documented in [the unified library](UNIFIED_LIBRARY.md#games-without-files).
**Still open:** a library-card preview dialog; the full game page remains current
behavior. This dated checklist does not close rendered/native/controller gates.

## Presentation goals still to validate or extend

Game Play now uses a consistent solid blue with contrast-adjusted ink for dark
and light themes, independent of cover hue and the music accent. Home/game-page
action groups share a 46 px height (54 px in Big Screen), common text sizing and
horizontal padding; icon-only Favourite uses the same height as its width.
Aircloud's compact secondary-button rule respects this group height, avoiding
the previous 40/46/54 px mismatch. Setup/acquisition actions retain their own
meaning rather than being styled as ready-to-play controls.

The cover-layout pass uses **Platform shelves** as the saved library layout,
with **Grid** retained in the layout selector. Shelves group the existing chosen
play option's platform without changing canonical identities; sorting/filtering
still determines the order within each system. Each shelf initially renders 12
games, with Show more and View all controls. View all selects that platform and
uses the paginated grid. Search, catalogue and single-platform views remain flat.
Grouping is therefore by the displayed installation, not a new game identity or
an assertion that each console has only one packaging shape.

Home shelves preserve recent-play/recent-add order. All shelf images share a
height and use variable widths; the decoded image supplies its actual ratio,
without cropping, with bounded width reservation. Platform case ratios are only
defaults for loading/missing-art states; regional packaging varies. In a filtered
grid, the platform default also reserves a more suitable row height. Missing-art
cards no longer repeat the game title both inside and beneath the cover.

**Settings → Appearance → Visual style** now offers **Aircloud Color** (default)
and **Classic**. The workspace dashboard's `web/src/theme.js` and `dashboard.css`
informed the slate surfaces, thin borders, compact 8–14 px controls/panels and
blue/teal palette. Marquee keeps its existing readable sans font, brighter game
accents and independently saved music color. Dark/light and effects preferences
remain independent; this is a Marquee adaptation, not an Aircloud runtime package
dependency. Cover frames remain hover/focus treatments. Rendered acceptance at
desktop/narrow/Big Screen sizes remains open.

The owner-selected game-page layout now follows a Steam-style media/details
structure without commerce or reviews: compact title, a large real-media stage
with selectable thumbnails, release information alongside it, then Play/manage
controls with local activity. Overview and OST remain below. The shared page
width aligns these sections; narrow windows stack the media and summary.
Cover-only games retain the cover's proportions in the stage. Missing images
show a labelled decorative placeholder; screenshots/trailers are not invented.
Installation/source details remain secondary. Gallery controls and native
desktop layout still require rendered keyboard/responsive acceptance.

The artwork/details pass adds a playtime/last-played/runner strip and a larger
cover alongside the game hero at desktop widths. Missing descriptions use an
honest library summary with artwork/metadata/music actions rather than leaving
technical matching as the main content. Identification stays in a secondary
disclosure. Find artwork is available for ROM entries without disc serials;
Genesis now participates in the existing hash-based libretro cover lookup.
An unchecked firmware label is suppressed for systems without that requirement.
Music artwork distinguishes embedded/local album images from linked-game covers.

The 2026-10-24 music shell pass aligns the sidebar/content above a full-width
player footer, using its measured height rather than unrelated fixed offsets.
Desktop navigation includes Music; Add games stays in the Games library header,
and Big Screen moves to the top-right toolbar. Narrow navigation sits above the
player. Music albums and the session queue share the OST playback state. Visual
checks at narrow/wide sizes and desktop input acceptance remain open.

Home shelves must use real library/play/readiness metadata. Canonical pages retain
edition/run selection, separate saves and provenance-labeled screenshots. Keep
source/platform/runtime/install filters distinct and paginate or virtualize large
libraries. Manager Downloads must display measured progress or an explicit handoff.
Dark/light tokens, reduced motion, restrained effects, idle animation pause and
clear focus remain shared Desktop/Big Screen goals; Big Screen needs larger
targets, controller glyphs and reliable Back/context behavior. Native Tauri input
must be verified independently of browser Gamepad API tests. Capture supported
sizes/themes/input modes; a build is not visual acceptance. Optional collections,
notes, save backup, Gamescope and streaming remain [expansion work](EXPANSION_CATALOGUE.md).
