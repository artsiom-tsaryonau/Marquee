# Hardening and maintenance plan

Created 2026-11-25 from a three-pass static audit of `main` at `fa6ae5b`.
This document owns security hardening, trust-boundary cleanup and maintenance
work that agents can execute independently. It does not reorder the
[roadmap](ROADMAP.md): product milestones, live acceptance gates and owner
priorities stay there. No item here closes a live gate.

Each work item below is self-contained: current behavior with file anchors, the
change, tests, acceptance and what is out of scope. Line numbers are as of
`fa6ae5b`; re-locate by symbol name if they have moved.

## Rules for executing agents

- Take one work item per change set. Do not bundle unrelated items; several
  items touch the same files (see [conflict map](#conflict-map)).
- Read the files named in the item before editing. Re-verify the "Current
  behavior" statement; if it is no longer true, stop and record that here.
- Preserve existing IDs, data paths, receipts, saves, settings, tokens and user
  edits. Never reset another contributor's changes.
- Run `scripts/check.sh` before handing back. A `BLOCKED` line for missing
  WebKit/DBus devel packages is acceptable only if you say so explicitly.
- Do not commit or push unless the owner asks.
- When an item is done, change its status in the [tracker](#tracker) and add a
  dated line under it with the commit, tests added and anything left open.
- Do not mark live behavior verified from unit tests. If an item asks for a
  manual check you cannot perform, say it is unperformed.

## Verified baseline (2026-11-25)

Facts the items depend on. Each was checked in code, not only in docs.

| Area | Fact | Where |
| --- | --- | --- |
| Loopback server | Binds `127.0.0.1` only (ports 47820–47829), rejects foreign `Host`, compares tokens in constant time | `crates/core/src/server.rs` `start`, `host_ok`, `same` |
| API auth | `authorized()` accepts the bearer header **or** a `?token=` query value, for both `/api/` and `/art/` | `server.rs:293-298`, `:336`, `:418` |
| Cover URLs | The browser builds `/art/<file>?token=<full API token>`; the desktop uses the Tauri asset protocol instead | `apps/desktop/src/lib/api/map.ts:85-98` |
| Media/music URLs | Use per-item `sha256(prefix:token:id)` keys; tests assert the token is absent and that a key does not authorize `/api/` | `server.rs:56-70`, tests near `:600-623` |
| Token power | The API can save and open an arbitrary executable (`save_launcher`, `open_launcher`), so the API token is equivalent to running code as the user | `crates/core/src/api.rs:424-431`, `crates/core/src/launchers.rs` |
| Desktop port | Without `MARQUEE_WEB`, the desktop still starts the **full** server (including `/api/` and `/art/`) on first music or game-media playback | `apps/desktop/src-tauri/src/commands.rs:29-59` |
| Release trust, pinned in repo | Xenia, Mesen, melonDS (`download` blocks); Shipwright and 2Ship2Harkinian (`pinned_release`); Purple and umu; Moonlight; yt-dlp; source builds pinned by commit and digest-pinned base images; Wolf by commit and image digests | `catalog/`, `purple.rs:21-25`, `music_discovery.rs:16-17`, `source_builds.rs` |
| Release trust, upstream "latest" | PCSX2, DuckStation (rolling), RPCS3, shadPS4, KytyPS5, Flycast, RMG, xemu, Azahar, PPSSPP, Cemu install whatever GitHub lists as latest; a GitHub `digest` is **required**; asset URL host is not checked | `crates/core/src/emulators.rs:500-558` |
| Release trust, ports | Zelda64Recomp, sm64coopdx, Fallout 2 CE, UZDoom install latest; a missing digest only logs "continuing over HTTPS"; asset URL host is not checked | `crates/manager/src/port_engine.rs:206-238`, `:307-318` |
| Download call sites | Five separate verify-after-download paths | `emulators.rs:624`, `port_engine.rs:291`, `purple.rs:514`, `streaming/receiver.rs:136`, `music_discovery.rs:153` |
| Receipt publication | Prepared jobs are registered in `library.db` only when someone calls `preparation_jobs` (UI 5 s poll or CLI `prepare list`) | `crates/core/src/preparation.rs:25-53`, `apps/desktop/src/App.svelte:80-84` |
| Systems in UI | `LibrarySnapshot` has no system descriptors; about 10 UI files hardcode system ids, and `map.ts` duplicates each system's default emulator from `catalog/systems/*.json` | `crates/core/src/model.rs:314-322`, `map.ts:11-17` |
| Catalog already exposed | `preparation_systems` returns only `id`/`name`; `SystemDescriptor` has `emulator` and `alternative_emulators` | `api.rs:312-316`, `crates/media/src/catalog.rs:24-39` |
| Dev URL params | Only `route`, `mode`, `theme`, `motion`, `splash` are read. `game`, `add`, `empty`, `demo` are not. There is no demo mode in `apps/desktop/src` | `App.svelte:37`, `:87-94` |
| Tests | 357 Rust `#[test]`s; 107 UI test cases in 9 files under `apps/desktop/tests`; plus `scripts/tests` | `rg '#\[test\]' crates` |
| CI | No `.github/` directory; README states there is no hosted CI. Remote is `github.com/artsiom-tsaryonau/Marquee` | `README.md:163` |
| Schema | Library `SCHEMA_VERSION` 6 | `crates/core/src/model.rs:4` |

## Corrections to the earlier same-day audit

Do not act on these earlier claims:

- **Private torrent listener on all interfaces is intended.** The private
  engine listens on `0.0.0.0:51413` so the owner's other devices can fetch
  shared games (`crates/manager/src/torrent.rs:570-590`). DHT, trackers and
  discovery stay off. A loopback default would break device sharing. Only
  item U2 (visibility) remains.
- **The catalogue page opener is safe.** `open_catalogue_page` passes a stored
  URL to Chrome or `xdg-open`, but import accepts only fixed
  `https://vimm.net/vault/…` and `https://www.axekin.com/games/…` forms
  (`crates/core/src/metadata.rs:159-170`, `:324-372`), so no argument or
  scheme injection is possible.
- **Digest presence is not the main release gap.** A GitHub API digest
  protects the CDN download but still trusts whatever upstream publishes at
  install time. The meaningful split is reviewed pin in this repository versus
  upstream latest (see H2 and decision D1).
- UI test count is 9 files with 107 cases, not 10 scripts.

## Owner decisions

Agents must not decide these. Items that depend on them say so.

| ID | Question | Decision, 2026-11-25 | Blocks |
| --- | --- | --- | --- |
| D1 | Should every downloaded executable be pinned to a reviewed SHA in this repo, or may emulators keep installing upstream latest with a required GitHub digest? | Accepted the recommendation: ports are pinned (H2). Emulators keep latest-with-digest plus the host check (H2b) | H2b scope |
| D2 | Add hosted CI on GitHub Actions, or keep local `scripts/check.sh` only? | Declined. The owner does not use GitHub Actions. Checks stay in `scripts/check.sh` | Q1 |
| D3 | Should the desktop's lazy media server stay a full API server, or be media-only? | Accepted: media-only (H4) | H4 |

## Tracker

| ID | Item | Priority | Size | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| H1 | Scoped cover capability; bearer only via header | P0 | S | — | Done in the working tree |
| H2 | Pin the four unpinned native ports; refuse unpinned ports without a digest | P0 | S–M | — | Done in the working tree |
| H2b | Check release asset URL host for emulators and ports | P1 | S | H2, D1 | Done in the working tree |
| H4 | Media-only lazy server for the desktop | P1 | S | H1, D3 | Done in the working tree |
| D-1 | Correct README claims (demo, dev params, ports, tests) | P1 | XS | H4 (for the port sentence) | Done in the working tree |
| Q1 | Hosted CI for the portable check subset | P1 | M | D2 | Cancelled. No GitHub Actions |
| A1 | Publish receipts from the backend, not from UI polling | P2 | M | — | Done in the working tree |
| H3 | One verified-download helper for all five call sites | P2 | M | H2, H2b | Done in the working tree |
| A2 | Systems in the library snapshot; drive UI defaults from the catalog | P2 | M | — | Done in the working tree |
| U1 | Show active downloads and preparation outside the archive panel | P2 | S | A1 (preferred) | Done in the working tree |
| H5 | Limit implicit `.env` discovery; repair token file mode | P3 | XS | — | Done in the working tree |
| U2 | Show the private sharing listener address in Device sharing | P3 | XS | — | Done in the working tree |

## Implementation log — 2026-11-25

The owner asked to implement the plan, so D1–D3 use the recommended defaults above.

Local checks that passed: `cargo fmt --all`, `cargo test --workspace --exclude marquee-desktop --locked`, `cargo clippy --workspace --exclude marquee-desktop --all-targets --locked -- -D warnings`, `node --test scripts/tests`, the Python `scripts/tests`, `pnpm test` and `pnpm check` in `apps/desktop`. `pnpm build` and a full `scripts/check.sh` were not run. No browser, desktop playback, or live port install was exercised.

- **H1.** `POST /api/art_key` returns `sha256("marquee-art:" + token)`. `/art/` accepts only `?key=`. `/api/` accepts only `Authorization: Bearer`. Browser `coverSrc` uses the key and stays empty until `loadArtKey` finishes. An existing group-readable `server.token` is repaired to mode 0600.
- **H2.** Zelda64Recomp v1.2.2, sm64coopdx v1.5.1, Fallout 2 CE v1.3.0 and UZDoom 5.0.3 are pinned, including the executable hash inside each archive. Fallout 2 CE's archive hash was computed locally because that GitHub release publishes no digest. A pinned install whose upstream tag differs reports `review` and does not offer Update. An unpinned recipe with no digest is refused.
- **H2b / H3.** `download_verified` requires HTTPS or an exact URL or a `github.com/{owner}/{repo}/releases/download/` URL, and a 64-hex SHA-256. Emulators, ports, Purple, Moonlight and yt-dlp use it. Emulator `pick` also rejects an asset URL outside the recipe repository.
- **H4.** The desktop's lazy playback server is `Surface::MediaOnly`. `/api/` and `/art/` on it return 404.
- **H5.** Implicit `.env` lookup runs in debug builds and inside this checkout. `MARQUEE_ENV_FILE` is unchanged. Release builds started elsewhere do not walk parent directories.
- **A1.** `Core::start_receipt_publisher` runs in the desktop process and `marquee run web`. A test prepares a Genesis archive and sees it registered without calling `preparation_jobs` first.
- **A2.** `LibrarySnapshot.systems` comes from `catalog::all()`. `pickRunner` uses that emulator id and keeps `SYSTEM_EMULATOR` only when the snapshot has no systems. `systemLabel` uses catalog display names after the snapshot arrives.
- **U1.** Games shows a count of preparing, review, interrupted and failed preparation jobs, active transfers, and waiting or downloading browser jobs. The count opens the existing downloads panel.
- **U2.** Settings shows the private listen address while transfers are enabled. The default remains `0.0.0.0:51413`.
- **Q1.** Cancelled after implementation. The owner does not use GitHub Actions, so no workflow file is kept. Run `scripts/check.sh` locally before pushing.
- **D-1.** README no longer describes `?demo=1`, invented titles, or a desktop that never opens a port.

## Waves

Items in one wave can run in parallel unless the conflict map says otherwise.

1. **Wave 1:** H1, H2, Q1 (after D2), H5.
2. **Wave 2:** H4 (after H1), H2b (after H2), A1, A2.
3. **Wave 3:** H3 (after H2b), U1 (after A1), D-1, U2.

## Conflict map

| File | Items |
| --- | --- |
| `crates/core/src/server.rs` | H1, H4 — serialize, H1 first |
| `apps/desktop/src-tauri/src/commands.rs` | H4 |
| `apps/desktop/src/lib/api/map.ts` | H1, A2 — serialize |
| `apps/desktop/src/lib/api/http.ts`, `contracts.ts`, `core.ts` | H1, A2, U1 — small, rebase carefully |
| `crates/manager/src/port_engine.rs` | H2, H2b, H3 — serialize in that order |
| `crates/core/src/emulators.rs` | H2b, H3 — serialize |
| `crates/media/src/http.rs` | H3 |
| `crates/core/src/preparation.rs`, `crates/core/src/lib.rs` | A1 |
| `crates/core/src/model.rs`, `api.rs` | A2 (and H1 if a command is added in `api.rs`; H1 should add it in `server.rs` instead) |
| `apps/desktop/src/App.svelte` | A1, U1 |
| `README.md` | D-1 only; other items record their notes here, not in README |

---

## H1 — Scoped cover capability

**Why.** The browser surface puts the full API token in every cover URL. Any
extension with request access, a copied image address or a devtools screenshot
discloses it, and the token can save and run an arbitrary launcher. Music and
media already solved this with derived keys.

**Current behavior.**
- `apps/desktop/src/lib/api/map.ts:94-97` returns
  `/art/<name>?token=<token>` for browser covers.
- `crates/core/src/server.rs:293-298` `authorized()` falls back to
  `query_value(query, "token")`. It is used for `/api/` (`:336`) and `/art/`
  (`:418`).
- Tests that rely on the query token:
  `instance_cache_covers_remain_visible_when_the_desktop_uses_xdg_cache`
  (`:659-681`) and `unknown_commands_and_art_traversal_are_rejected` (`:684-704`).
- Nothing in the UI sends `?token=` to `/api/`; `http.ts` always uses the
  `Authorization` header, including `/api/events`.

**Change.**
1. In `server.rs`, add `fn art_key(token: &str) -> String` returning
   `sha256("marquee-art:{token}")` as hex, following `music_key`/`media_key`.
   One key for the art directory is enough: covers are not sensitive, and it
   keeps `coverSrc` synchronous.
2. Handle `POST /api/art_key` in `server.rs` beside `media_url`/`music_url`,
   behind `authorized()`, returning the key as a JSON string. Do not add it to
   `api::dispatch`; the desktop has no token and does not need it.
3. `/art/<name>`: accept only `?key=<art_key>` compared with `same()`. Remove
   the token path for art.
4. `authorized()`: accept only `Authorization: Bearer`. Delete the query
   fallback.
5. `apps/desktop/src/lib/api/http.ts`: fetch the art key once after the token is
   known (for example inside `webToken()` consumers or on first `coverSrc`
   use via a module-level cache filled during app boot). Export a synchronous
   getter.
6. `map.ts` `coverSrc`: in the browser, build `/art/<name>?key=<artKey>`.
   Until the key is loaded return `undefined` (the existing fallback art
   handles this), and emit a re-render once it arrives (for example by
   re-applying the snapshot or a reactive store in `state.svelte.ts`).
7. Optional in the same change: in `load_or_create_token`, if an existing
   token file is group/world readable, `chmod` it to `0600`.

**Tests.**
- Replace the two server tests' `?token=` art requests with `?key=`.
- New: `/art/x?token=<token>` returns 401 or 403.
- New: `POST /api/health?token=<token>` without a header returns 401.
- New: the art key does not authorize `POST /api/health` as a bearer (mirror the
  music-key test near `:620`).
- New: `art_key` response does not contain the token.
- UI: a unit test in `apps/desktop/tests` that `coverSrc` in web mode never
  contains the token string (stub `webToken` and the art key).

**Acceptance.**
- `rg -n 'token=' apps/desktop/src crates/core/src/server.rs` finds only the
  `#token=` handoff URL and its parsing.
- Browser surface (`scripts/run.sh both`): covers load, and the image request
  URL contains `key=` and not the token. Record if this manual check was not
  performed.
- Desktop covers are unchanged (still asset protocol).

**Out of scope.** Rotating existing tokens; per-file art keys; changing the
`#token=` handoff.

## H2 — Pin the four unpinned native ports

**Why.** Shipwright and 2Ship2Harkinian install an exact reviewed asset and
SHA-256. The other four ports unpack whatever GitHub lists as latest, and
proceed even when the release has no digest. The unpacked binary is executed.

**Current behavior.**
- `catalog/ports/{zelda64recomp,sm64coopdx,fallout2-ce,uzdoom}.json` have no
  `pinned_release`.
- `port_engine.rs:206-238` picks the first asset matching `asset_suffix`, takes
  `browser_download_url` unchecked and `digest` optionally.
- `port_engine.rs:307-318`: `None => progress("This release publishes no
  checksum; continuing over HTTPS.")`.
- `validate_pinned_binary` (`:372`) and the pinned path in `pick_asset`
  (`:153-204`) already implement the strict flow.

**Change.**
1. For each of the four ports, choose the release currently installed or
   recommended (check `port_dir` receipts on the owner's machine only if asked;
   otherwise the current latest). Using
   `gh api repos/<github>/releases/tags/<tag>`, record `tag`, exact `asset`
   name and the asset `digest`. If GitHub has no digest, download the asset,
   compute SHA-256 locally and say so in the item log.
2. Add `pinned_release` to each JSON, including `binary_sha256` for the
   executable inside the archive (compute by extracting locally) where the
   recipe has a `binary`. For UZDoom (`binary: null`, AppImage) use the asset
   hash, as Shipwright does.
3. In `port_engine.rs`, replace the `None =>` "continuing over HTTPS" branch with
   an error (`PORT_RELEASE`, "This release publishes no checksum; the recipe
   needs a reviewed pin"). This keeps any future unpinned recipe from silently
   installing.
4. Keep the update check (`Core::check_port_updates`, `crates/core/src/lib.rs:511`)
   reporting newer upstream tags; with a pin it must report "review needed",
   not install.

**Tests.**
- Catalog test (in `crates/manager/src/port_engine.rs` tests or
  `crates/core/src/catalog.rs`): every `catalog/ports/*.json` has a
  `pinned_release` with a 64-hex `sha256` and an exact asset name.
- `pick_asset` with an unpinned def and an asset without `digest` returns an
  error.
- Existing pinned tests (`:826-850`) still pass.

**Acceptance.**
- Fresh isolated install of one changed port, for example
  `cargo run -p marquee-cli -- --data-dir /tmp/mq-h2 ports install sm64coopdx`,
  completes and the receipt records the pinned tag. Network required; record
  if skipped.
- An already installed port at the same tag is not reinstalled.

**Out of scope.** Game data setup; changing which upstream repository a port
uses; emulators (H2b, D1).

## H2b — Check release asset URL host

**Depends on:** H2, owner decision D1 (scope).

**Why.** For upstream-latest emulators and any future unpinned recipe, the
download URL comes straight from the GitHub API response. Requiring it to point
at the recipe's own repository is cheap defense in depth and matches the
pinned-port rule.

**Current behavior.** `emulators.rs:553-555` and `port_engine.rs:217` accept any
`browser_download_url`.

**Change.**
1. Add one shared function (in `crates/media/src/http.rs` or a small new module
   used by both crates):
   `fn github_release_asset(url: &str, repo: &str) -> bool` requiring
   `https://github.com/{repo}/releases/download/` as a case-insensitive prefix,
   no userinfo, no port, no `..` segments.
2. Call it in `emulators::pick` and in the unpinned branch of
   `port_engine::pick_asset`; reject otherwise.
3. `def.download.url` pins in `catalog/emulators/*.json` already point at
   GitHub; assert that in a catalog test as well.

**Tests.** Accept the real pattern; reject other hosts, other repositories,
`http://`, `https://github.com.evil/…`, `https://user@github.com/…`. Existing
fixtures using `"u1"`-style URLs must be updated to realistic URLs.

**Acceptance.** `check_emulator_updates` and one real emulator install
(for example PCSX2 in an isolated `--data-dir`) still work. Record if skipped.

**Out of scope.** Pinning emulators (D1).

## H4 — Media-only lazy server for the desktop

**Depends on:** H1 (same file), owner decision D3.

**Why.** README says the desktop "opens no port by default". In fact the first
music or game-media playback starts the full server, which also serves `/api/`
and `/art/`. The desktop only needs `/music/` and `/media/`.

**Current behavior.** `apps/desktop/src-tauri/src/commands.rs:29-59` calls
`server::start(core, None, None)` and stores the handle in `music_server`.

**Change.**
1. Add a surface parameter to `server::start`, for example
   `enum Surface { Full, MediaOnly }`. Keep `start` behavior for existing
   callers (web, CLI) as `Full`.
2. In `MediaOnly`, `handle()` serves only `/music/` and `/media/`; everything
   else returns 404 before any auth check.
3. Use `MediaOnly` in `commands.rs` `music_url` and `media_url`.
4. Consider binding the media-only server to an ephemeral port (`Some(0)` is
   not supported by the current 10-port loop; add explicit support) so it never
   collides with a later `MARQUEE_WEB` server on 47820. Optional.

**Tests.** Media-only server: `/music/<id>?key=…` works; `/api/health` with a
valid bearer returns 404; `/art/x` returns 404; unknown UI paths return 404.

**Acceptance.** Desktop plays an OST track and a game video. Record if not
performed.

## D-1 — Correct README claims

**Depends on:** H4 for the port sentence.

**Change in `README.md`:**
- Line 155: replace "opens no port by default" with the actual behavior after
  H4 (a loopback media-only port opens when music or game media plays; the full
  library API needs `MARQUEE_WEB` or `MARQUEE_PORT`).
- Line 157: remove the "Demo (`pnpm dev`, no token, or `?demo=1`)" paragraph, or
  replace it with what `pnpm dev` without a token now does (the HTTP API shows
  the "Open the full address…" error, `http.ts:50-57`).
- Line 161: list only implemented dev params (`route`, `mode`, `theme`,
  `splash`, `motion`). Also update the comment at `App.svelte:87`.
- Line 163: if Q1 lands, mention CI; otherwise leave it.
- If counts are added, use the baseline numbers above.

**Acceptance.** Every dev parameter in README is read in `App.svelte`; no
README sentence describes a demo mode. Run the docs checks named in
[the roadmap](ROADMAP.md#verification-and-handoff) (links, whitespace).

## Q1 — Hosted CI for the portable check subset

**Status:** Cancelled 2026-11-25. The owner does not use GitHub Actions. Do not add `.github/workflows/`. `scripts/check.sh` remains the check.

**Depends on:** D2.

**Why.** `scripts/check.sh` is thorough but runs only when someone remembers.
The tree has 357 Rust tests and 107 UI cases.

**Change.**
1. Add `.github/workflows/check.yml` on `push` and `pull_request` to `main`,
   `ubuntu-latest`, with Rust stable (the repo has no `rust-toolchain` file;
   pin a version in the workflow if clippy drift becomes noisy), Node 22, pnpm
   via corepack, and cache for cargo and pnpm.
2. Steps mirror `scripts/check.sh`: node and Python `scripts/tests`, `cargo fmt
   --check`, `cargo clippy --workspace --exclude marquee-desktop --all-targets
   --locked -- -D warnings`, `cargo test --workspace --exclude marquee-desktop
   --locked`, then `pnpm install --frozen-lockfile`, `pnpm check`, `pnpm test`,
   `pnpm build` in `apps/desktop`.
3. Install `libwebkit2gtk-4.1-dev`, `libdbus-1-dev` and Tauri build deps and run
   `cargo check -p marquee-desktop --locked` as a separate job, so a WebKit
   packaging problem does not hide core failures.
4. Prefer calling `scripts/check.sh` directly if it runs unchanged on Ubuntu;
   otherwise keep it the local source of truth and note differences in a comment
   at the top of the workflow.

**Risks to check.** Tests that need Fedora-only tools, Podman, `/proc` boot ID,
`flatpak` or network. If any fail on Ubuntu for environmental reasons, gate them
with an explicit `#[ignore]` reason or environment check, list them in this
item's log, and do not weaken the assertion.

**Acceptance.** A pushed branch shows the workflow green, or the failing tests
are listed here with cause. Update README line 163.

## A1 — Publish receipts from the backend

**Why.** A prepared game becomes a library entry only when some client calls
`preparation_jobs`. Correctness depends on a UI being open and polling every
5 s (`App.svelte:80-84`), whose result is discarded. `marquee run web` with no
tab open never registers finished jobs.

**Current behavior.** `Core::preparation_jobs` maps `reconcile_preparation` over
`self.preparation.list()` (`preparation.rs:47-53`). `reconcile_preparation`
publishes `AwaitingLibrary` receipts and acknowledges them to the manager.

**Change.**
1. Add `Core::start_receipt_publisher(self: &Arc<Self>)` that runs on a
   background thread: every 5 s, if the manager is running, call
   `preparation_jobs()` and ignore errors (log once per error code). Stop it when
   the last `Arc<Core>` drops (use a `Weak` in the thread).
2. Start it from the desktop setup (`apps/desktop/src-tauri/src/lib.rs`) and from
   `marquee run web` in the CLI. Do not start it in `Core::open_isolated` used by
   tests unless a test opts in.
3. Keep `preparation_jobs` idempotent; it already guards duplicate publication
   via `preparation_receipts` and fingerprints.
4. Change the `App.svelte` poll to a display refresh only (or remove it if U1
   subscribes to events instead). `LibraryChanged` is already emitted on
   publication.

**Tests.** Extend an existing preparation test: complete a job, start the
publisher with no client calls, wait for registration, assert `edition_ids`
present and the job acknowledged once. Assert a second `preparation_jobs()`
call does not duplicate.

**Acceptance.** With `marquee run web` and no browser tab, a CLI-prepared archive
appears in the library after the publisher interval.

**Out of scope.** Changing manager protocol or receipt format.

## H3 — One verified-download helper

**Depends on:** H2, H2b.

**Why.** Five call sites (`emulators.rs:624`, `port_engine.rs:291`,
`purple.rs:514`, `streaming/receiver.rs:136`, `music_discovery.rs:153`) each
download, then compare hashes in their own way. The port gap came from one of
them diverging.

**Change.**
1. In `crates/media/src/http.rs`, add
   `download_verified(url, expected_sha256, allowed: UrlRule, dest, max_bytes,
   timeout, progress) -> Result<()>` that: requires HTTPS; checks `UrlRule`
   (exact URL, GitHub release prefix for a repository, or an explicit host);
   streams to `dest`; compares lowercase hex; removes `dest` on any failure.
   `expected_sha256` is a required argument, not `Option`.
2. Migrate the five call sites. Keep their user-facing error codes and messages.
3. Make the unverified `download_file` crate-private or rename it
   `download_unverified` with a doc comment, so new callers choose explicitly.

**Tests.** Unit tests for the helper with the existing loopback server pattern
(`http.rs:157-179`): wrong hash removes the file; non-HTTPS rejected (allow a
test-only loopback override); rule mismatch rejected. Existing call-site tests
stay green.

**Acceptance.** `rg -n 'download_file\(' crates` shows only the helper and
deliberate unverified uses.

## A2 — Systems in the library snapshot

**Why.** Adding a platform requires edits in about 10 UI files, and
`map.ts:11-17` duplicates each system's default emulator, which
`catalog/systems/*.json` already owns. Drift leads to wrong default runners.

**Change.**
1. Add `systems: Vec<SystemSummary>` to `LibrarySnapshot`
   (`crates/core/src/model.rs:314`), with `id`, `display_name`, `emulator`,
   `alternative_emulators`, `supported_extensions`. Populate it from
   `catalog::all()` in `Core::snapshot`. PS1 and PS2 are catalog systems too
   (`catalog/systems/ps1.json`, `ps2.json`) but use the `duckstation`/`pcsx2`
   runner kinds; keep their existing runner matching in `runnerSupports`.
2. Mirror the type in `apps/desktop/src/lib/api/contracts.ts`.
3. In `map.ts`, derive the default emulator in `pickRunner` from
   `snap.systems`; keep `SYSTEM_EMULATOR` only as a fallback for old fixtures,
   marked as such.
4. Use `display_name` as the fallback label in `labels.ts` where a system has no
   explicit UI label.
5. Keep icons and the `GameEntry['platform']` union UI-owned; this item does not
   remove them.

**Tests.** Rust: snapshot includes every catalog system with its emulator.
UI: `state.test.mjs` / map tests pick the catalog default; an unknown system
gets its `display_name` label.

**Acceptance.** Removing an entry from `SYSTEM_EMULATOR` does not change default
runner selection for snapshots that include `systems`.

## U1 — Visible downloads and preparation

**Depends on:** A1 preferred (so the UI no longer owns publication).

**Why.** Preparation, torrent and browser jobs exist, but their progress is
only visible inside Games → Downloads and archives. The roadmap and README note
there is no persistent downloads surface.

**Change.**
1. In `state.svelte.ts`, keep a small `activity` summary: counts of preparation
   jobs in `preparing`/`awaiting_review`/`interrupted`/`failed`, active torrent
   downloads and active browser jobs. Refresh on the existing poll or on
   `LibraryChanged` plus a slow interval.
2. Show a count badge on the Games nav item (`App.svelte` `nav`) and a one-line
   status near the "Downloads and archives" entry (`Library.svelte:180`).
   Clicking opens the existing panel.
3. Respect reduced motion; no new top-level route.

**Tests.** UI unit test for the summary reducer (states to counts).

**Acceptance.** Manual: start a preparation job, see the badge, open the panel
from it. Record if not performed.

## H5 — Limit implicit `.env` discovery

**Why.** `KeyStore::dotenv_values` walks up to six ancestors of the current
directory for `.env` (`crates/core/src/keys.rs:170-183`). A release desktop
started from `$HOME` can read an unrelated `~/.env`.

**Change.** Only search ancestors in debug builds (`cfg!(debug_assertions)`) or
when the cwd is inside this repository; always honor `MARQUEE_ENV_FILE`.
Document in README "Secrets".

**Tests.** Existing isolated tests unchanged; add one for the explicit file path.

## U2 — Show the private sharing listener

**Why.** The private torrent engine listens on all interfaces at port 51413 by
design. The user should see that when enabling device sharing.

**Change.** In Settings → Device sharing, show the listen address from
`transfer_status` and a short note that devices on the same network can connect
when they know a share link. No behavior change.

## Final verification sweep (for the last agent in a wave)

1. `scripts/check.sh` passes (or reports only `BLOCKED` for desktop deps).
2. `rg -n '\?token=' apps/desktop/src crates` returns nothing except tests that
   assert rejection.
3. `rg -n 'continuing over HTTPS' crates` returns nothing.
4. Every `catalog/ports/*.json` has `pinned_release`.
5. README dev params match `App.svelte`.
6. This tracker's statuses and dated logs match what landed.
