#!/usr/bin/env python3
"""
Minimal sample: open a Vimm's Vault game in a new tab of your running browser.

Playwright does not control Cursor's built-in browser — use Chrome, Chromium, or Edge
with remote debugging enabled.

  uv sync --project scripts/vimm --locked
  uv run --project scripts/vimm python -m playwright install chromium   # only for --launch

  # 1) Start a separate automation profile (ordinary Chrome can stay open):
  google-chrome --remote-debugging-port=9222 --user-data-dir="$HOME/.local/share/marquee/browser-poc"

  # 2) Run (opens a new tab in that window):
  uv run --project scripts/vimm scripts/vimm/download_one_sample.py

  # Optional: another game URL, different CDP port, or isolated Playwright Chromium:
  python scripts/vimm/download_one_sample.py https://vimm.net/vault/1808
  python scripts/vimm/download_one_sample.py --cdp http://127.0.0.1:9223
  python scripts/vimm/download_one_sample.py --launch

By default the script only opens the tab; you click Download and Continue yourself.
Use --auto-click to let Playwright press Download (still click Continue if prompted).
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

from playwright.sync_api import TimeoutError as PlaywrightTimeout
from playwright.sync_api import sync_playwright

from vimm_browser import DEFAULT_CDP_URL, attach_playwright, resolve_cdp_url
from vimm_parse import blocked_var_name

GAME_URL = "https://vimm.net/vault/43937"  # Addams Family Values (Genesis)
OUT_DIR = Path(__file__).resolve().parents[2] / "data" / "vimm" / "sample-download"
WAIT_AD_CHECK_S = 25.0
DOWNLOAD_TIMEOUT_MS = 600_000


def wait_for_ad_check(page, timeout_s: float) -> bool:
    var = blocked_var_name(page.content())
    if not var:
        return True
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        try:
            if page.evaluate(f"() => typeof {var} !== 'undefined' && {var} === false"):
                return True
        except Exception:
            pass
        page.wait_for_timeout(500)
    return False


def run_download_flow(page, *, out_dir: Path, auto_click: bool) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)

    if auto_click:
        if not wait_for_ad_check(page, WAIT_AD_CHECK_S):
            print("Ad check still active — click Download manually if needed.")
        submit = page.locator("#dl-form button[type='submit']")
        if not submit.count():
            raise RuntimeError("No Download button on this page.")
        with page.expect_download(timeout=DOWNLOAD_TIMEOUT_MS) as dl_info:
            submit.click()
            try:
                page.wait_for_selector("#dialog3[open]", timeout=3_000)
                print("Dialog open — click Continue in the browser tab …")
                page.wait_for_function(
                    "() => !document.getElementById('dialog3')?.open",
                    timeout=DOWNLOAD_TIMEOUT_MS,
                )
            except PlaywrightTimeout:
                pass
        download = dl_info.value
    else:
        print("In the new tab: click Download, then Continue if the site asks.")
        download = page.wait_for_event("download", timeout=DOWNLOAD_TIMEOUT_MS)

    name = download.suggested_filename or "download.bin"
    dest = out_dir / name
    download.save_as(dest)
    return dest


def main() -> int:
    parser = argparse.ArgumentParser(description="Download one Vault game via your browser")
    parser.add_argument("url", nargs="?", default=GAME_URL, help="Vault game page URL")
    parser.add_argument(
        "--cdp",
        default=None,
        help=f"Chrome DevTools URL (default {DEFAULT_CDP_URL}). Set empty with --launch.",
    )
    parser.add_argument(
        "--launch",
        action="store_true",
        help="Launch a separate Playwright Chromium instead of attaching to your browser",
    )
    parser.add_argument(
        "--auto-click",
        action="store_true",
        help="Click the Download button for you (you may still need to click Continue)",
    )
    parser.add_argument("-o", "--output-dir", type=Path, default=OUT_DIR)
    parser.add_argument(
        "--keep-tab",
        action="store_true",
        help="Leave the game tab open after download (default: close tab when done)",
    )
    args = parser.parse_args()

    cdp_url = None if args.launch else (resolve_cdp_url(args.cdp) or DEFAULT_CDP_URL)

    print(f"Game page: {args.url}")
    print(f"Save to:   {args.output_dir.resolve()}")
    if cdp_url:
        print(f"Browser:   existing window via {cdp_url} (new tab)")

    with sync_playwright() as p:
        attachment = attach_playwright(p, cdp_url=cdp_url, headless=False)
        page = attachment.new_tab()
        try:
            page.goto(args.url, wait_until="domcontentloaded", timeout=60_000)
            page.wait_for_selector("#dl-form", timeout=60_000)
            dest = run_download_flow(page, out_dir=args.output_dir, auto_click=args.auto_click)
            print(f"Saved {dest}")
        except PlaywrightTimeout:
            print("Timed out waiting for download.", file=sys.stderr)
            return 1
        except RuntimeError as exc:
            print(exc, file=sys.stderr)
            return 1
        finally:
            if args.keep_tab:
                attachment.shutdown(None)
            else:
                attachment.shutdown(page)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
