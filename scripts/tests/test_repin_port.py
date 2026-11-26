"""The port re-pin helper writes a reviewed checksum and refuses a bad digest."""

import hashlib
import io
import json
import threading
import unittest
import zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
ELF = b"\x7fELF" + b"\x00" * 14 + (62).to_bytes(2, "little") + b"\x00" * 44


class Handler(BaseHTTPRequestHandler):
    payload = ELF
    digest = "sha256:" + hashlib.sha256(ELF).hexdigest()
    asset_name = "Game.AppImage"

    def do_GET(self):
        if self.path.endswith("/asset"):
            body = self.payload
        else:
            body = json.dumps(
                {
                    "tag_name": "v9",
                    "assets": [
                        {
                            "name": self.asset_name,
                            "browser_download_url": f"http://127.0.0.1:{self.server.server_address[1]}/asset",
                            "digest": self.digest,
                        }
                    ],
                }
            ).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *_args):
        return


class RepinTests(unittest.TestCase):
    def setUp(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.port = self.server.server_address[1]

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    def recipe(self, directory: Path) -> Path:
        path = directory / "demo.json"
        path.write_text(
            json.dumps(
                {
                    "id": "demo",
                    "github": "example/demo",
                    "asset_suffix": ".appimage",
                    "asset_excludes": ["mac"],
                    "binary": None,
                    "pinned_release": {
                        "tag": "v1",
                        "asset": "old",
                        "sha256": "ab" * 32,
                        "binary_sha256": "cd" * 32,
                    },
                }
            )
        )
        return path

    def run_script(self, directory: Path):
        return subprocess.run(
            [
                "python3",
                str(ROOT / "repin-port.py"),
                "demo",
                "v9",
                "--catalog",
                str(directory),
                "--api-base",
                f"http://127.0.0.1:{self.port}",
            ],
            check=False,
            text=True,
            capture_output=True,
        )

    def test_a_matching_release_rewrites_the_pin(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            self.recipe(directory)
            result = self.run_script(directory)
            self.assertEqual(result.returncode, 0, result.stderr)
            written = json.loads((directory / "demo.json").read_text())
            self.assertEqual(written["pinned_release"]["tag"], "v9")
            self.assertEqual(written["pinned_release"]["sha256"], hashlib.sha256(ELF).hexdigest())
            self.assertIn("pinned_release", result.stdout)

    def test_a_wrong_digest_leaves_the_recipe_unchanged(self):
        Handler.digest = "sha256:" + "ab" * 32
        try:
            with tempfile.TemporaryDirectory() as raw:
                directory = Path(raw)
                before = self.recipe(directory).read_text()
                result = self.run_script(directory)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((directory / "demo.json").read_text(), before)
        finally:
            Handler.digest = "sha256:" + hashlib.sha256(ELF).hexdigest()

    def test_a_nested_binary_is_hashed_separately_from_the_archive(self):
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as archive:
            archive.writestr("Game/engine", ELF)
        Handler.payload = buffer.getvalue()
        Handler.asset_name = "Game.zip"
        Handler.digest = "sha256:" + hashlib.sha256(Handler.payload).hexdigest()
        try:
            with tempfile.TemporaryDirectory() as raw:
                directory = Path(raw)
                path = self.recipe(directory)
                recipe = json.loads(path.read_text())
                recipe["asset_suffix"] = ".zip"
                recipe["binary"] = "engine"
                path.write_text(json.dumps(recipe))
                result = self.run_script(directory)
                self.assertEqual(result.returncode, 0, result.stderr)
                written = json.loads(path.read_text())
                self.assertEqual(written["pinned_release"]["sha256"], hashlib.sha256(Handler.payload).hexdigest())
                self.assertEqual(written["pinned_release"]["binary_sha256"], hashlib.sha256(ELF).hexdigest())
                self.assertNotEqual(written["pinned_release"]["sha256"], written["pinned_release"]["binary_sha256"])
        finally:
            Handler.payload = ELF
            Handler.asset_name = "Game.AppImage"
            Handler.digest = "sha256:" + hashlib.sha256(ELF).hexdigest()


if __name__ == "__main__":
    unittest.main()
