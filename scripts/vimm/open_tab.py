#!/usr/bin/env python3
"""
Open a tab in your normal Chrome session, with no debugging setup.

  uv run --project scripts/vimm scripts/vimm/open_tab.py https://vimm.net/vault
  uv run --project scripts/vimm scripts/vimm/open_tab.py --profile 'Profile 1'

Use --cdp URL only to attach to an already configured debugging browser.
"""

from __future__ import annotations

import argparse
import subprocess
import sys

from vimm_browser import attach_playwright, open_native_tab


def main() -> int:
    parser = argparse.ArgumentParser(description="Open a tab in your ordinary Chrome session")
    parser.add_argument(
        "url",
        nargs="?",
        default="https://vimm.net/vault",
        help="Page to open",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--cdp", help="Optional Chrome DevTools endpoint (automation profile)")
    mode.add_argument("--browser", help="Chrome/Chromium executable name or path")
    parser.add_argument("--profile", help="Chrome profile directory name, e.g. 'Default' or 'Profile 1'")
    args = parser.parse_args()
    if args.cdp and args.profile:
        parser.error("--profile selects a native Chrome profile; it cannot be used with --cdp")

    try:
        if not args.cdp:
            open_native_tab(args.url, browser=args.browser, profile=args.profile)
            print(f"Tab requested in Chrome: {args.url}")
            return 0

        from playwright.sync_api import sync_playwright

        with sync_playwright() as p:
            attachment = attach_playwright(p, cdp_url=args.cdp)
            page = attachment.new_tab()
            page.goto(args.url, wait_until="domcontentloaded", timeout=60_000)
            print(f"New tab: {args.url}")
            print("Done — tab stays open in your browser.")
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        print(f"Could not open tab: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
