# Browser / Vault proof of concept

## Working sample flow

The export also integrates with Marquee: generate it with `sample_flow.py build`,
then open Library → Catalogue metadata and choose `.poc/catalog.json`. See
[catalogue metadata integration](../../docs/CATALOGUE_METADATA.md) for regional
filters, source artwork/ratings, explicit installation links and SHA-1 matching.

From the repository root, start the local catalogue and open it in your normal
Chrome profile:

```sh
uv run --project scripts/vimm --locked scripts/vimm/sample_flow.py serve --open
```

Use `--browser google-chrome --profile 'Profile 1'` to choose a Chrome profile.
Without `--open`, the command prints a loopback URL to open yourself. The server
binds only to `127.0.0.1`; port 0 selects a free port. Ctrl+C stops it.

1. Browse the 36 systems. Genesis has 43 saved A-section games; other systems
   explicitly show that no catalogue page was saved.
2. Search for **Addams**, select **Europe** and language **fr**, and choose
   **Addams Family Values**.
3. Inspect media ID **36950**, version **1.0**, filename, size and recorded hashes.
   The sample has one media variant; the picker supports multiple variants.
4. Click **Open live page in a new tab**. The link opens through the browser
   displaying the POC, using that browser's session. It does not promise the saved
   media is still available or automatically select the same variant remotely.
5. Choose another catalogue game to see the explicit missing-detail state.

The three required input files are `Vimm's Lair_ The Vault.html`, `The Vault -
Genesis - A.html` and `The Vault_ Addams Family Values (Genesis).html` under
`sample/`. The extra Genesis overview HTML is not required. Source HTML scripts,
third-party requests and saved form tokens are never served or replayed. Only
whitelisted catalogue/media metadata is exported.

To generate a standalone HTML preview and JSON export without a server:

```sh
uv run --project scripts/vimm --locked scripts/vimm/sample_flow.py build
```

Outputs go to ignored `scripts/vimm/.poc/index.html` and `catalog.json`. The HTML
also works as a local file opened manually. Override with `--sample-dir` and
`--output-dir`. Missing/changed/mismatched input pages fail visibly rather than
silently generating an empty successful result. This is a standalone POC, not
an automated download connector. Its exported catalogue metadata is integrated
into the Marquee Library and game details.

Tests:

```sh
uv run --project scripts/vimm --locked python -m unittest discover -s scripts/vimm -p 'test_*flow.py' -v
uv run --project scripts/vimm --locked python -m playwright install chromium --only-shell
uv run --project scripts/vimm --locked scripts/vimm/test_sample_ui.py
```

The UI test owns a disposable headless browser, stubs the external page, and
checks browsing, search/filtering, variant details, the new-tab link, missing
snapshots and narrow viewport layout. It never connects to personal Chrome.

Keep this POC in Python. Browser startup, page loading and network waits dominate
the work; there is no measured CPU bottleneck that warrants a Rust rewrite.
Python has an official Playwright client and the existing parser already works
against the saved samples. Rust remains a good fit for Marquee's job queue,
library database, import validation and native browser-launch command.

## Run with uv

From the Marquee repository root:

```sh
uv sync --project scripts/vimm --locked
uv run --project scripts/vimm --locked scripts/vimm/open_tab.py https://vimm.net/vault
```

The normal opener uses Chrome/Chromium's `--new-tab` command and its ordinary
profile. Chrome forwards the request to the running instance for that profile;
if it is not running, Chrome may start. No debugging port, Playwright-managed
browser, profile copying or browser restart is required. Successful command
delivery is not proof the website loaded. If multiple browser profiles/windows
are open, use the internal profile directory name to select the profile; this
does not select a particular window within that profile.

```sh
uv run --project scripts/vimm --locked scripts/vimm/open_tab.py --browser google-chrome --profile 'Profile 1' https://vimm.net/vault
```

The native opener also works with `python3` directly, without third-party
dependencies. This launcher implementation targets Linux Chrome/Chromium;
Flatpak-only installations and other operating systems need their own launcher
adapters. `VIMM_CDP_URL` deliberately does not change the native opener's mode.

`uv` still creates an isolated `.venv`; it manages creation, synchronization and
the interpreter command for you. `pyproject.toml` declares dependencies and
`uv.lock` records the resolution. Update intentionally with `uv lock --project
scripts/vimm --upgrade`, then rerun the offline checks. The old manually managed
pip requirements file has been replaced.

## Opening a tab versus controlling it

The native opener requests a normal tab only. It does not read its DOM, click
buttons, observe downloads or identify the created tab.

The existing collection/download tools use Playwright. CDP attachment requires a
browser that already exposes a debugging endpoint. Since Chrome 136, the
`--remote-debugging-port` flag requires a non-default user-data directory:

```sh
google-chrome --remote-debugging-port=9222 --user-data-dir="$HOME/.local/share/marquee/browser-poc"
uv run --project scripts/vimm --locked scripts/vimm/open_tab.py --cdp http://127.0.0.1:9222 https://vimm.net/vault
```

This creates a separate automation profile, not attachment to the normal default
profile. Keep that distinction visible in Marquee. CLI `--cdp` overrides
`VIMM_CDP_URL`; download tools fall back to `http://127.0.0.1:9222`. Only
`--launch` needs the downloaded Playwright Chromium executable:

```sh
uv run --project scripts/vimm --locked python -m playwright install chromium
```

For future interaction inside the everyday Chrome session, use a small optional
Manifest V3 extension plus a Marquee native messaging host. The extension can
create tabs and report their IDs, communicate with permitted page content
scripts, and report browser download events. Scope site access explicitly and
operate only on tabs/jobs created or selected for Marquee. Keep manual browser
handoff usable without the extension. This is a proposed architecture, not
implemented functionality. Changing Python to Rust would not remove the need
for a browser integration mechanism.

## Review findings

Fixed in this pass:

- Normal-session tab opening no longer requires CDP or imports Playwright.
- Download CLI defaults no longer mask the `VIMM_CDP_URL` environment variable.
- Debugging setup instructions account for modern Chrome's profile restriction.
- Dependencies are locked and local Python cache/environment files are ignored.

Before promoting the downloader into a supported Marquee connector:

1. **Durable download identity and state.** `load_done_ids` / `save_done_ids` in
   `vimm_collect.py` key only by game ID. A second disc, version or format gets
   skipped. Store source ID + media/disc/version/format, destination, byte count,
   status and errors. Write state atomically, reject corrupt state visibly and
   confirm the destination still exists before skipping a completed job. At
   present corrupt state silently becomes an empty set and writes are not atomic.
2. **Accurate completion and cancellation.** `cmd_download` catches timeouts and
   ultimately returns success even if every attempted download timed out. Return
   a summary and nonzero exit status for failures. Put cleanup in `finally` so
   failures or cancellation close owned tabs/browsers but preserve the user's
   other tabs. Browser ownership separation in `BrowserAttachment` is already a
   useful foundation.
3. **Reuse a session.** `fetch_html_playwright` starts and stops Chromium for
   every catalogue section. Reuse one browser/context per run. Replace its fixed
   1.5-second sleep with a wait for a known page element or an explicit
   unavailable/error state. Add bounded retry/backoff for transient failures.
4. **Explicit page outcomes.** Waiting for `#dl-form` cannot distinguish a
   removed/unavailable game from a changed page or navigation failure. Separate
   these outcomes; preserve the site's manual interaction when required. Extract
   the duplicated single-download/ad-check flow into one adapter.
5. **Deterministic storage.** Validate/sanitize all remote filename components,
   define collision behavior, stage downloads and rename after successful
   completion. Resolve output/state paths consistently instead of varying with
   the caller's working directory. Treat imported CSV URLs as untrusted and
   constrain automated collection to the intended provider origins.
6. **Real parser regression tests.** `test-parse` currently prints counts and
   succeeds when samples are missing or selectors return no records. Add small
   synthetic fixtures with assertions for empty/unavailable pages, metadata
   variants, relative links, duplicate IDs and malformed embedded media JSON.
   `parse_all_media` assumes the inline JavaScript array is valid JSON and its
   regex matches the page format; handle failure as a provider parse error.
7. **Connector boundary.** Keep site selectors/parsing in the provider adapter,
   browser attachment in a browser service, and jobs/import in Marquee core.
   Prefer structured JSON events over parsing progress text. A supported
   connector needs cancellation, progress and restart recovery before bulk use.

Do not rewrite all of this in Rust first. Stabilize the browser and job contracts,
then move the generic launcher/job/import pieces into `marquee-core`. Keep the
Python adapter as a development worker until distribution or maintenance costs
justify replacing it.

## Verification

```sh
uv run --project scripts/vimm --locked python -m unittest discover -s scripts/vimm -p test_browser.py -v
uv run --project scripts/vimm --locked scripts/vimm/vimm_collect.py test-parse --sample-dir sample
```

Six browser-contract tests pass using mocks: literal argv/profile handling,
HTTP(S)-only native opening, missing executable errors, shell-free dispatch,
CDP endpoint precedence and attached-browser preservation. Saved sample smoke
checks parse 36 systems, 43 Genesis A-list records and one media entry. These
counts are observations, not an asserted parser regression suite. No live ROM
downloads or manual-session browser interactions were performed in this review.

Sources: [uv project workflow](https://docs.astral.sh/uv/guides/projects/),
[Chrome debugging changes](https://developer.chrome.com/blog/remote-debugging-port),
[Chrome tabs API](https://developer.chrome.com/docs/extensions/reference/api/tabs),
[native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging),
[Playwright CDP support](https://playwright.dev/python/docs/api/class-browsertype#browser-type-connect-over-cdp).
