#!/usr/bin/env python3
"""
Catalog and download games from Vimm's Lair The Vault (POC automation).

Uses a real browser for downloads because the site allows one file at a time and
runs an ad-block check before each download. Use --headed (default) and click
Continue when the dialog appears if the check has not cleared yet.

Setup:
  uv sync --project scripts/vimm --locked
  uv run --project scripts/vimm python -m playwright install chromium

Examples:
  # Systems table (all consoles / handhelds on /vault)
  python scripts/vimm/vimm_collect.py systems -o data/vimm/systems.csv

  # Full Genesis catalog (#, A–Z)
  python scripts/vimm/vimm_collect.py catalog --system Genesis -o data/vimm/genesis.csv

  # Download first 3 games from a catalog (browser window opens)
  python scripts/vimm/vimm_collect.py download --catalog data/vimm/genesis.csv --out ~/Games/vimm/genesis --limit 3

  # Offline parser check against saved pages in sample/
  python scripts/vimm/vimm_collect.py test-parse --sample-dir sample

  # One-file download in a new tab of your Chrome (remote debugging on port 9222)
  google-chrome --remote-debugging-port=9222 --user-data-dir="$HOME/.local/share/marquee/browser-poc"
  python scripts/vimm/download_one_sample.py
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
import time
from pathlib import Path
from typing import Any
from urllib.parse import urljoin

from playwright.sync_api import TimeoutError as PlaywrightTimeout
from playwright.sync_api import sync_playwright

from vimm_browser import DEFAULT_CDP_URL, attach_playwright, resolve_cdp_url
from vimm_parse import (
    VaultListEntry,
    blocked_var_name,
    catalog_to_csv_rows,
    decode_good_title,
    load_sample_catalog,
    parse_all_media,
    parse_list_page,
    parse_systems,
    parse_vault_sections,
    read_html_file,
    systems_to_csv_rows,
)

BASE_URL = "https://vimm.net"
DEFAULT_DELAY_S = 2.5


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not rows:
        path.write_text("", encoding="utf-8")
        return
    with path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        writer.writeheader()
        writer.writerows(rows)


def read_catalog(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def fetch_html_playwright(url: str, *, headless: bool, timeout_ms: int) -> str:
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=headless)
        page = browser.new_page()
        page.goto(url, wait_until="domcontentloaded", timeout=timeout_ms)
        page.wait_for_timeout(1500)
        html = page.content()
        browser.close()
    return html


def section_url(system: str, section: str) -> str:
    label = section
    if label == "#":
        return f"{BASE_URL}/vault/?p=list&system={system}&section=number"
    if label:
        return f"{BASE_URL}/vault/{system}/{label}"
    return f"{BASE_URL}/vault/{system}"


def cmd_systems(args: argparse.Namespace) -> int:
    if args.from_file:
        html = read_html_file(Path(args.from_file))
    else:
        html = fetch_html_playwright(
            f"{BASE_URL}/vault", headless=args.headless, timeout_ms=args.timeout_ms
        )
    systems = parse_systems(html, base_url=BASE_URL)
    rows = systems_to_csv_rows(systems)
    write_csv(Path(args.output), rows)
    print(f"Wrote {len(rows)} systems to {args.output}")
    return 0


def cmd_catalog(args: argparse.Namespace) -> int:
    system = args.system
    sections: list[tuple[str, str]] = []
    all_entries: list[VaultListEntry] = []
    seen: set[str] = set()

    if args.from_file:
        path = Path(args.from_file)
        html = read_html_file(path)
        label = ""
        if args.sections:
            label = args.sections.split(",")[0].strip()
        else:
            og = re.search(
                r'property="og:url"\s+content="[^"]*/vault/[^/]+/([^"?]+)',
                html,
            )
            if og:
                label = og.group(1)
            elif "section=number" in html:
                label = "#"
        page_url = section_url(system, label) if label else f"{BASE_URL}/vault/{system}"
        for entry in parse_list_page(html, system=system, section=label, page_url=page_url):
            if entry.game_id not in seen:
                seen.add(entry.game_id)
                all_entries.append(entry)
    else:
        if args.sections:
            for s in args.sections.split(","):
                s = s.strip()
                if s:
                    sections.append((s, section_url(system, s)))
        else:
            menu_html = fetch_html_playwright(
                f"{BASE_URL}/vault/{system}",
                headless=args.headless,
                timeout_ms=args.timeout_ms,
            )
            for sec in parse_vault_sections(menu_html):
                if sec.label:
                    sections.append((sec.label, urljoin(BASE_URL, sec.path)))

        for label, url in sections:
            print(f"Fetching {url} …")
            html = fetch_html_playwright(url, headless=args.headless, timeout_ms=args.timeout_ms)
            time.sleep(args.delay)

            for entry in parse_list_page(html, system=system, section=label, page_url=url):
                if entry.game_id in seen:
                    continue
                seen.add(entry.game_id)
                all_entries.append(entry)

    rows = catalog_to_csv_rows(all_entries)
    out = Path(args.output)
    write_csv(out, rows)
    print(f"Wrote {len(rows)} games to {out}")
    return 0


def wait_for_ad_check(page, timeout_s: float) -> bool:
    var = blocked_var_name(page.content())
    if not var:
        return True
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            ok = page.evaluate(f"() => typeof {var} !== 'undefined' && {var} === false")
            if ok:
                return True
        except Exception:
            pass
        page.wait_for_timeout(500)
    return False


def apply_download_options(page, args: argparse.Namespace) -> None:
    if args.format is not None:
        sel = page.query_selector("#dl_format")
        if sel and sel.is_visible():
            sel.select_option(str(args.format))
    if args.version:
        sel = page.query_selector("#dl-version")
        if sel and sel.is_visible():
            sel.select_option(label=args.version)
    if args.disc is not None:
        sel = page.query_selector("#disc-number")
        if sel and sel.is_visible():
            sel.select_option(str(args.disc))


def download_one_game(
    page,
    *,
    url: str,
    out_dir: Path,
    game_id: str,
    title: str,
    args: argparse.Namespace,
) -> Path | None:
    page.goto(url, wait_until="domcontentloaded", timeout=args.timeout_ms)
    page.wait_for_selector("#dl-form", timeout=args.timeout_ms)

    apply_download_options(page, args)

    if args.wait_ad_check > 0:
        if not wait_for_ad_check(page, args.wait_ad_check):
            print(
                "  Ad-block check still active — click Download, then Continue in the browser.",
                file=sys.stderr,
            )

    submit = page.locator("#dl-form button[type='submit']")
    if not submit.count():
        print(f"  No download button for {game_id}", file=sys.stderr)
        return None

    out_dir.mkdir(parents=True, exist_ok=True)

    with page.expect_download(timeout=args.download_timeout_ms) as dl_info:
        submit.click()
        try:
            page.wait_for_selector("#dialog3[open]", timeout=3_000)
            if args.manual_continue:
                print("  Dialog open: click Continue in the browser window …")
                page.wait_for_function(
                    "() => !document.getElementById('dialog3')?.open",
                    timeout=args.download_timeout_ms,
                )
            else:
                page.locator("#dialog3 input[type='submit'][value='Continue']").click()
        except PlaywrightTimeout:
            pass

    download = dl_info.value
    suggested = download.suggested_filename or f"{game_id}.zip"
    safe_title = "".join(c if c.isalnum() or c in " ._-" else "_" for c in title)[:80].strip()
    dest = out_dir / f"{game_id}_{safe_title}_{suggested}"
    download.save_as(dest)
    return dest


def load_done_ids(state_path: Path) -> set[str]:
    if not state_path.is_file():
        return set()
    try:
        data = json.loads(state_path.read_text(encoding="utf-8"))
        return set(str(x) for x in data.get("downloaded_game_ids", []))
    except Exception:
        return set()


def save_done_ids(state_path: Path, done: set[str]) -> None:
    state_path.parent.mkdir(parents=True, exist_ok=True)
    state_path.write_text(
        json.dumps({"downloaded_game_ids": sorted(done)}, indent=2) + "\n",
        encoding="utf-8",
    )


def cmd_download(args: argparse.Namespace) -> int:
    rows = read_catalog(Path(args.catalog))
    if args.limit:
        rows = rows[: args.limit]

    done = load_done_ids(Path(args.state_file))
    out_dir = Path(args.output).expanduser()

    cdp_url = None if args.launch else (resolve_cdp_url(args.cdp) or args.cdp or DEFAULT_CDP_URL)
    if args.headless and cdp_url:
        print("Note: --headless is ignored when attaching via --cdp.", file=sys.stderr)

    with sync_playwright() as p:
        attachment = attach_playwright(
            p,
            cdp_url=cdp_url,
            headless=args.headless,
            slow_mo=args.slow_mo,
        )
        page = attachment.new_tab()

        for i, row in enumerate(rows, 1):
            game_id = row.get("game_id") or ""
            if not game_id:
                continue
            if game_id in done and not args.redownload:
                print(f"[{i}/{len(rows)}] skip {game_id} (already downloaded)")
                continue
            url = row.get("url") or f"{BASE_URL}/vault/{game_id}"
            title = row.get("title") or game_id
            print(f"[{i}/{len(rows)}] {title} ({game_id})")
            try:
                path = download_one_game(
                    page,
                    url=url,
                    out_dir=out_dir,
                    game_id=game_id,
                    title=title,
                    args=args,
                )
            except PlaywrightTimeout as exc:
                print(f"  Timeout: {exc}", file=sys.stderr)
                continue
            if path:
                print(f"  Saved {path}")
                done.add(game_id)
                save_done_ids(Path(args.state_file), done)
            time.sleep(args.delay)

        attachment.shutdown(page)
    return 0


def cmd_test_parse(args: argparse.Namespace) -> int:
    sample_dir = Path(args.sample_dir)
    vault_html = sample_dir / "Vimm's Lair_ The Vault.html"
    if vault_html.is_file():
        systems = parse_systems(read_html_file(vault_html))
        print(f"systems: {len(systems)} from {vault_html.name}")
        for s in systems[:3]:
            print(f"  - {s.slug}: {s.name} ({s.launch_year})")

    list_html = sample_dir / "The Vault - Genesis - A.html"
    if list_html.is_file():
        entries = parse_list_page(
            read_html_file(list_html),
            system="Genesis",
            section="A",
            page_url="https://vimm.net/vault/Genesis/A",
        )
        print(f"list A: {len(entries)} games from {list_html.name}")
        if entries:
            print(f"  first: {entries[0].title} -> {entries[0].url}")

    game_html = sample_dir / "The Vault_ Addams Family Values (Genesis).html"
    if game_html.is_file():
        raw = read_html_file(game_html)
        media = parse_all_media(raw)
        print(f"game page: {len(media)} media entries, blocked={blocked_var_name(raw)}")
        if media:
            print(f"  filename hint: {decode_good_title(media[0].get('GoodTitle'))}")

    entries = load_sample_catalog(sample_dir)
    print(f"sample catalog total: {len(entries)}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Vimm's Lair Vault catalog / download POC")
    p.add_argument("--headless", action="store_true", help="Run browser without UI")
    p.add_argument("--timeout-ms", type=int, default=60_000)
    p.add_argument("--delay", type=float, default=DEFAULT_DELAY_S, help="Pause between HTTP pages")

    sub = p.add_subparsers(dest="command", required=True)

    sp = sub.add_parser("systems", help="Fetch /vault and write systems CSV")
    sp.add_argument("-o", "--output", default="data/vimm/systems.csv")
    sp.add_argument("--from-file", help="Parse saved HTML instead of live fetch")
    sp.set_defaults(func=cmd_systems)

    cp = sub.add_parser("catalog", help="Crawl list pages for one system")
    cp.add_argument("--system", required=True, help="Vault slug, e.g. Genesis, PS2")
    cp.add_argument("-o", "--output", default="data/vimm/catalog.csv")
    cp.add_argument("--sections", help="Comma-separated section labels (#,A,B,…) — default all")
    cp.add_argument("--from-file", help="Use one saved list HTML (A page) only")
    cp.set_defaults(func=cmd_catalog)

    dp = sub.add_parser("download", help="Download games one-by-one from catalog CSV")
    dp.add_argument("--catalog", required=True)
    dp.add_argument("--output", required=True, help="Download directory")
    dp.add_argument("--state-file", default="data/vimm/download-state.json")
    dp.add_argument("--limit", type=int, default=0)
    dp.add_argument("--redownload", action="store_true")
    dp.add_argument("--format", type=int, choices=[0, 1, 2], help="dl_format index if present")
    dp.add_argument("--version", help="dl-version label if dropdown visible")
    dp.add_argument("--disc", type=int, help="disc-number if present")
    dp.add_argument("--wait-ad-check", type=float, default=15.0, help="Seconds to wait for ad script")
    dp.add_argument(
        "--manual-continue",
        action="store_true",
        default=True,
        help="Wait for you to click Continue on the ad dialog (default on)",
    )
    dp.add_argument("--no-manual-continue", action="store_false", dest="manual_continue")
    dp.add_argument("--download-timeout-ms", type=int, default=300_000)
    dp.add_argument("--slow-mo", type=int, default=0, help="Playwright slow_mo ms for debugging")
    dp.add_argument(
        "--cdp",
        default=None,
        help="Attach to your browser via Chrome DevTools (new tab). Default port 9222.",
    )
    dp.add_argument(
        "--launch",
        action="store_true",
        help="Launch isolated Playwright Chromium instead of attaching to your browser",
    )
    dp.set_defaults(func=cmd_download)

    tp = sub.add_parser("test-parse", help="Parse HTML under sample/")
    tp.add_argument("--sample-dir", default="sample")
    tp.set_defaults(func=cmd_test_parse)

    return p


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
