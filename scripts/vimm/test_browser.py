"""Browser contract tests: never launch or attach to a real browser."""

import os
import unittest
from unittest.mock import Mock, patch

from vimm_browser import attach_playwright, native_tab_command, open_native_tab, resolve_cdp_url


class BrowserTests(unittest.TestCase):
    @patch("vimm_browser.shutil.which", return_value="/usr/bin/google-chrome")
    def test_manual_session_uses_normal_profile_and_literal_arguments(self, which):
        url = "https://example.org/?title=a&value=$(touch%20bad)"
        command = native_tab_command(url, profile="Profile 1")
        self.assertEqual(command, ["/usr/bin/google-chrome", "--profile-directory=Profile 1", "--new-tab", url])
        self.assertFalse(any("remote-debugging" in arg or "user-data-dir" in arg for arg in command))

    def test_only_web_urls_are_accepted(self):
        for url in ["--new-window", "file:///etc/passwd", "javascript:alert(1)", "https://"]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                native_tab_command(url)

    @patch("vimm_browser.shutil.which", return_value=None)
    def test_missing_browser_has_actionable_error(self, which):
        with self.assertRaisesRegex(RuntimeError, "--browser"):
            native_tab_command("https://example.org")

    @patch("vimm_browser.subprocess.run")
    @patch("vimm_browser.shutil.which", return_value="/opt/chrome")
    def test_opener_does_not_use_a_shell(self, which, run):
        open_native_tab("https://example.org", browser="/opt/chrome")
        run.assert_called_once_with(["/opt/chrome", "--new-tab", "https://example.org"], check=True, timeout=15)

    @patch.dict(os.environ, {"VIMM_CDP_URL": "http://127.0.0.1:9223"})
    def test_explicit_endpoint_wins_then_environment(self):
        self.assertEqual(resolve_cdp_url(None), "http://127.0.0.1:9223")
        self.assertEqual(resolve_cdp_url("http://127.0.0.1:9224"), "http://127.0.0.1:9224")

    @patch("vimm_browser.cdp_reachable", return_value=True)
    def test_attached_session_cleanup_preserves_browser(self, reachable):
        playwright = Mock()
        browser = playwright.chromium.connect_over_cdp.return_value
        browser.contexts = [Mock()]
        attachment = attach_playwright(playwright, cdp_url="http://127.0.0.1:9222")
        tab = attachment.new_tab()
        attachment.shutdown(tab)
        tab.close.assert_called_once()
        browser.close.assert_not_called()


if __name__ == "__main__":
    unittest.main()
