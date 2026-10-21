import json
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import urlopen

from sample_flow import FILES, ROOT, build, create_server, load_snapshot, render_html


def write_fixtures(root: Path):
    pages = {
        "systems": '<table class="rounded"><caption>Consoles</caption><tr><td><a href="/vault/Genesis">Genesis</a></td><td class="yearColumn"><span>1988</span></td></tr></table>',
        "catalog": '<meta property="og:url" content="https://vimm.net/vault/Genesis/A"><table class="hovertable"><tbody><tr><td><a href="/vault/43937">Addams Family Values</a></td><td><img class="flag" title="Europe"></td><td>1.0</td><td>de en fr</td><td>8</td></tr></tbody></table>',
        "detail": '<meta property="og:url" content="https://vimm.net/vault/43937"><script>let allMedia=[{"ID":36950,"Version":"1.0","GoodTitle":"Z2FtZS5tZA==","ZippedText":"1 MB"}];</script><input name="token" value="DO_NOT_EXPORT_TOKEN">',
    }
    for role, html in pages.items():
        (root / FILES[role]).write_text(html, encoding="utf-8")


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        write_fixtures(self.root)

    def tearDown(self):
        self.temp.cleanup()

    def test_links_system_catalog_and_media_without_exporting_tokens(self):
        data = load_snapshot(self.root)
        self.assertEqual(data["systems"][0]["sample_games"], 1)
        self.assertEqual(data["games"][0]["media"][0]["filename"], "game.md")
        self.assertEqual(data["detail_game_id"], "43937")
        self.assertNotIn("DO_NOT_EXPORT_TOKEN", json.dumps(data))

    def test_missing_sample_is_an_error(self):
        (self.root / FILES["detail"]).unlink()
        with self.assertRaisesRegex(ValueError, "Missing detail"):
            load_snapshot(self.root)

    def test_changed_catalog_selector_is_an_error(self):
        path = self.root / FILES["catalog"]
        path.write_text(path.read_text().replace("hovertable", "changed"))
        with self.assertRaisesRegex(ValueError, "no games"):
            load_snapshot(self.root)

    def test_mismatched_detail_is_an_error(self):
        path = self.root / FILES["detail"]
        path.write_text(path.read_text().replace("43937", "12345"))
        with self.assertRaisesRegex(ValueError, "does not match"):
            load_snapshot(self.root)

    def test_malformed_media_is_an_error(self):
        path = self.root / FILES["detail"]
        path.write_text(path.read_text().replace('"ID":36950', '"ID":invalid'))
        with self.assertRaisesRegex(ValueError, "malformed media"):
            load_snapshot(self.root)

    def test_external_provider_url_is_rejected(self):
        path = self.root / FILES["catalog"]
        path.write_text(path.read_text().replace('href="/vault/43937"', 'href="https://elsewhere.test/vault/43937"'))
        with self.assertRaisesRegex(ValueError, "Unexpected provider"):
            load_snapshot(self.root)

    def test_export_is_self_contained_and_escapes_embedded_markup(self):
        data = load_snapshot(self.root)
        data["games"][0]["title"] = '</script><script>alert("bad")</script>'
        build(data, self.root / "built")
        html = (self.root / "built/index.html").read_text()
        self.assertNotIn('</script><script>alert', html)
        self.assertIn('\\u003c/script>', html)
        self.assertNotIn("__SNAPSHOT_JSON__", html)
        self.assertEqual(json.loads((self.root / "built/catalog.json").read_text()), data)

    def test_http_serves_only_generated_outputs(self):
        data = load_snapshot(self.root)
        with create_server(data) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            url = f"http://127.0.0.1:{server.server_port}"
            try:
                with urlopen(url + "/catalog.json") as response:
                    self.assertEqual(json.load(response), data)
                with urlopen(url) as response:
                    self.assertIn(b"Explore the saved Vault catalogue", response.read())
                with self.assertRaises(HTTPError) as error:
                    urlopen(url + "/../sample/anything.html")
                self.assertEqual(error.exception.code, 404)
            finally:
                server.shutdown()
                thread.join()

    @unittest.skipUnless((ROOT / "sample" / FILES["detail"]).is_file(), "Local saved samples not present")
    def test_real_three_samples(self):
        data = load_snapshot(ROOT / "sample")
        self.assertEqual(len(data["systems"]), 36)
        self.assertEqual(len(data["games"]), 43)
        game = next(g for g in data["games"] if g["game_id"] == "43937")
        self.assertEqual(game["title"], "Addams Family Values")
        self.assertEqual(game["media"][0]["media_id"], "36950")
        self.assertEqual(game["media"][0]["filename"], "Addams Family Values (Europe) (En,Fr,De).md")
        self.assertEqual(game["players"], 1)
        self.assertIsNone(game["year"])
        self.assertEqual(game["rating_votes"], 2)
        self.assertEqual(game["ratings"]["overall"], 10)
        self.assertEqual(game["verified_at"], "2026-10-18")
        self.assertEqual(game["media"][0]["regions"], ["Europe"])
        self.assertEqual(game["media"][0]["languages"], ["en", "fr", "de"])
        self.assertTrue(game["cover"]["data_url"].startswith("data:image/avif;base64,"))


if __name__ == "__main__":
    unittest.main()
