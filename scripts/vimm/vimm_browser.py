"""Attach Playwright to an existing Chromium-based browser via CDP."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from typing import Any
from urllib.error import URLError
from urllib.request import urlopen
from urllib.parse import urlsplit

DEFAULT_CDP_URL = "http://127.0.0.1:9222"


def native_tab_command(
    url: str, *, browser: str | None = None, profile: str | None = None
) -> list[str]:
    """Build a Linux Chromium launcher command without touching profile data."""
    parsed = urlsplit(url)
    if parsed.scheme not in {"https", "http"} or not parsed.hostname:
        raise ValueError("Expected an absolute HTTP(S) URL")
    candidates = [browser] if browser else ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"]
    executable = next((path for name in candidates if (path := shutil.which(name))), None)
    if not executable:
        raise RuntimeError("Chrome/Chromium not found. Use --browser /path/to/browser.")
    command = [executable]
    if profile:
        command.append(f"--profile-directory={profile}")
    return [*command, "--new-tab", url]


def open_native_tab(url: str, *, browser: str | None = None, profile: str | None = None) -> None:
    # Chrome forwards this request to its running process for the same profile.
    # No shell, debugging port, cookie copying or second user-data directory.
    subprocess.run(native_tab_command(url, browser=browser, profile=profile), check=True, timeout=15)


@dataclass
class BrowserAttachment:
    browser: Any
    context: Any
    owns_browser: bool

    def new_tab(self) -> Any:
        return self.context.new_page()

    def shutdown(self, page: Any | None = None) -> None:
        if page is not None:
            try:
                page.close()
            except Exception:
                pass
        if self.owns_browser:
            try:
                self.browser.close()
            except Exception:
                pass


def cdp_reachable(cdp_url: str, timeout_s: float = 1.0) -> bool:
    base = cdp_url.rstrip("/")
    try:
        with urlopen(f"{base}/json/version", timeout=timeout_s) as resp:
            return resp.status == 200
    except (URLError, OSError, TimeoutError):
        return False


def cdp_startup_hint() -> str:
    return (
        "Start your browser with remote debugging, then run this script again:\n"
        '  google-chrome --remote-debugging-port=9222 --user-data-dir="$HOME/.local/share/marquee/browser-poc"\n'
        "Verify: curl -s http://127.0.0.1:9222/json/version\n"
        "Chrome 136+ requires a non-default data directory for these debugging flags.\n"
        "This is a separate profile, not your ordinary Chrome session.\n"
        "To only open a normal-session tab, use open_tab.py without --cdp."
    )


def resolve_cdp_url(explicit: str | None) -> str | None:
    if explicit:
        return explicit
    env = os.environ.get("VIMM_CDP_URL", "").strip()
    return env or None


def attach_playwright(
    playwright: Any,
    *,
    cdp_url: str | None,
    headless: bool = False,
    slow_mo: int = 0,
    accept_downloads: bool = True,
) -> BrowserAttachment:
    if cdp_url:
        if not cdp_reachable(cdp_url):
            print(cdp_startup_hint(), file=sys.stderr)
            raise SystemExit(f"Cannot reach browser at {cdp_url}")
        browser = playwright.chromium.connect_over_cdp(cdp_url)
        if browser.contexts:
            context = browser.contexts[0]
        else:
            context = browser.new_context(accept_downloads=accept_downloads)
        return BrowserAttachment(browser=browser, context=context, owns_browser=False)

    browser = playwright.chromium.launch(headless=headless, slow_mo=slow_mo)
    context = browser.new_context(accept_downloads=accept_downloads)
    return BrowserAttachment(browser=browser, context=context, owns_browser=True)
