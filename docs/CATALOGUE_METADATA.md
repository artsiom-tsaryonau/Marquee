# Catalogue metadata and regional revisions

Marquee accepts a version-1 Vimm metadata JSON export generated from the three
saved HTML samples. The same Rust API powers native Tauri and browser surfaces.
Importing metadata does not install a game, add a launchable edition, merge
canonical games, or alter an emulator profile.

## Use

```sh
uv run --project scripts/vimm --locked scripts/vimm/sample_flow.py build
```

Open Library → **Catalogue metadata** → select
`scripts/vimm/.poc/catalog.json`. Re-importing updates provider records by
`vimm:<game_id>` without duplicating them. Each request is limited to 900 KB;
larger catalogues need multiple exports/batches. Imports accumulate records;
absence from a later batch does not delete older entries.

Listing-only refreshes (`has_detail: false`) update listing fields while retaining
previously imported media variants, artwork, players, year, detail ratings and
verification dates for the same platform. A full detail snapshot remains
authoritative: it can remove variants and invalidate their links. A platform
change does not inherit the previous platform's details.

New imports record field-level provenance in the record's `provenance` map:
canonical provider page, export date, backend import date and `saved_snapshot`
method. **Field provenance** in the details view exposes that evidence. Retained
detail fields keep their own older dates when a newer listing arrives. Older
records acquire provenance when re-imported; capture times and verified
cross-provider identity are not inferred from export dates or artwork credits.

The Library has a Catalogue source filter, platform categories, region,
language, ROM revision and player-count filters. Search includes those metadata
labels and serials. Source rating sorting is independent of critic scores.
All known systems in the supplied index have display labels; a catalogue label
does not mean that system has a launch adapter. Genesis discovery currently has
43 entries; Marquee's local disc imports/launches still support PS1 and PS2.

Open Addams Family Values to see its saved cover, media variant, filename,
advertised size, checksums, players, source ratings and vote count, verification
date, provider identity, export/import dates and artwork credit. Unknown release
year remains unknown. The original sample box image is bundled as a bounded
raster data URL, with its attribution; arbitrary source HTML, scripts, form
tokens and external image fetching are not used. There are no screenshots or
developer/publisher/genre fields in this snapshot; none are invented.

**Open source in browser** opens the stored canonical page using normal Chrome
tab dispatch when available, otherwise the desktop's default browser opener.
It does not automate a download or claim live availability. Explicit Chrome
profile selection remains available through the standalone Python opener; the
Marquee button currently uses the browser's default profile.

## Metadata links and file matching

Catalogue entries and concrete installations have different IDs. On a discovery
detail page, choose a saved media revision and an existing installation for the
same platform, then explicitly confirm its identity to link metadata. This
manual link is labelled as such. It is not proof that the file has the recorded
checksum. Unlinking restores the installation's previous metadata projection;
it does not delete files, favorites, sessions or artwork already fetched locally.

Alternatively, on an existing PS1/PS2 installation page use **Match raw ISO by
SHA-1**. Only an exact, unique match on the same platform is linked. No title or
filename similarity matching is used. No match or an ambiguous match leaves the
installation unchanged. Hashing is explicit, streamed off the UI thread and
outside the database lock; it is never performed during browsing or startup.
The current command has no progress/cancellation UI and may take time on a
large ISO. It supports raw ISO bytes only, not CHD, CUE track sets or archives.
CRC32/MD5 are displayed as source metadata; they are not alternative automatic
match methods yet. Checksums identify the corresponding source content, not
necessarily its compressed download container.

Links remember the selected provider media ID, local media ID, size, mtime and
matching method. Changed/missing files, changed primary media, removed variants,
platform mismatches and changed source SHA-1 values invalidate applicable links.
The details view reports a stale link and offers removal/re-matching. File
size/mtime checks detect ordinary changes; this is not continuous content
verification or protection against intentionally preserved filesystem timestamps.

ROM revisions, regions and languages are properties of media variants. They
remain separate from content editions such as Anniversary and from launch
methods such as emulation/native. Variant region/language labels extracted from
GoodTitle filenames are derived metadata. Linked installations retain their
local locale labels when the selected variant has none; catalogue-wide labels
do not establish a particular installation's region or language. Discovery
filters retain catalogue-level fallback where variant labels are absent.
Filters evaluate region/language/revision together on
one variant, rather than combining attributes from unrelated variants.

Source ratings remain on their 0–10 scale with available vote counts. Source
verification dates are attributed claims. `captured_at` in the export currently
records export time, not a certified time when the HTML was originally saved;
Marquee labels this **Exported**. Import time is assigned by the Rust backend.
System launch years are shown as platform information and never substituted for
an unknown game release year.

## Persistence and API

Schema v5 adds `catalogue_records`, `catalogue_systems` and `catalogue_links`.
Records use a typed, validated JSON payload; links reference existing edition
IDs. The migration is additive and transactional. It does not rewrite editions,
media, launch profiles or sessions. Future normalized game/edition migrations
must use the next available schema number after v5.

Downloaded serial JPEG covers now pass bounded decoding (4096 pixels per side,
64 MiB decode allocation, 8 MiB download) before atomic publication. Refresh
retries corrupt cache files. Jobs recheck the installation identity and existing
artwork row before publication, so a rescan or intervening artwork correction
discards stale results. These protections apply to downloaded serial JPEGs;
embedded catalogue PNG/JPEG/AVIF validation still checks signatures and size.

Remaining enrichment work: persisted cross-provider mappings with explicit
acceptance evidence; exact artwork asset URLs and per-variant associations;
general field/artwork override editing and refresh precedence. Artwork credit
alone is not a verified release mapping. Live provider requests remain deferred.

Commands: `import_catalogue`, `link_catalogue`, `unlink_catalogue`,
`match_catalogue`, `clear_catalogue`, `open_catalogue_page`. Library snapshots
include `catalogue.records`, `catalogue.systems` and `catalogue.links`.

CLI examples (close an app sharing the same data directory before using CLI):

```sh
cargo run -p marquee-cli -- catalogue import scripts/vimm/.poc/catalog.json
cargo run -p marquee-cli -- catalogue list
cargo run -p marquee-cli -- catalogue match EDITION_ID
cargo run -p marquee-cli -- catalogue link EDITION_ID vimm:GAME_ID MEDIA_ID
cargo run -p marquee-cli -- catalogue unlink EDITION_ID
```

## Verification

Rust regression tests cover idempotent import, invalid input atomicity,
checksum/URL/image validation, unique/ambiguous/missing hash matches, manual
linking, stale files, schema-v4 migration, persistence, and preservation of
personal titles, favorites and playtime. Frontend tests cover nonplayable
discovery, per-install metadata projection, stale-link exclusion and correlated
variant filters. Python tests check the actual three saved samples.

The optional browser E2E uses an actual isolated Rust HTTP server, built Marquee
UI and disposable Chromium profile. It imports the JSON, filters by region,
language/revision/players, opens details, checks cover decoding and provenance,
checks reload persistence and removes the import. Browser launch requests are
stubbed, so the test cannot open a personal browser or download source media.

```sh
cargo build -p marquee-cli --locked
pnpm --dir apps/desktop build
uv run --project scripts/vimm --locked scripts/vimm/test_marquee_integration.py
```

Validated on 2026-10-21: repository checks passed (127 Rust tests, 47 frontend
tests, four launcher tests, Clippy, native compilation, zero Svelte diagnostics
and production build). Fifteen Python tests and both the standalone sample UI
and actual Marquee/Rust-backend browser E2E flows passed. The production native
app opened with accelerated shared-memory rendering. Importing the actual 43
sample records migrated the user library from v4 to v5 after a SQLite backup
under its `backups/` directory; before/after fingerprints confirmed that the two
roots, 11 editions/media entries, launch profiles and three session rows were
unchanged. No installation links were guessed for the Genesis sample.
