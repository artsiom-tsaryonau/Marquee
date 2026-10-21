"""Real Marquee HTTP/UI integration using a disposable DB and browser profile."""
import argparse
import json
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time
from urllib.parse import urlsplit, parse_qs
from urllib.request import Request, urlopen

from playwright.sync_api import sync_playwright, expect
from sample_flow import ROOT, build, load_snapshot


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/marquee")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="marquee-catalogue-e2e-") as temp:
        root = Path(temp)
        build(load_snapshot(ROOT / "sample"), root / "export")
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0)); port = sock.getsockname()[1]
        with (root / "server.log").open("w") as log:
            process = subprocess.Popen([str(args.binary), "--data-dir", str(root / "library"), "run", "web", "--port", str(port), "--web-dir", str(ROOT / "apps/desktop/dist")], stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 20
                match = None
                while time.monotonic() < deadline:
                    match = re.search(r"Marquee web UI: (http://[^\s]+)", (root / "server.log").read_text())
                    if match: break
                    if process.poll() is not None: raise RuntimeError("Disposable Marquee server exited before startup")
                    time.sleep(.1)
                if not match: raise RuntimeError("Disposable Marquee server did not start")
                address = urlsplit(match.group(1))
                token = parse_qs(address.fragment)["token"][0]
                # Do not import this machine's personal Lutris library into the test.
                config = {"enabled":False,"include_owned":False,"database_path":None,"launcher_id":"detected.lutris"}
                request = Request(f"http://{address.netloc}/api/configure_lutris", data=json.dumps({"config":config}).encode(), headers={"Authorization":f"Bearer {token}","Content-Type":"application/json"})
                with urlopen(request) as response: response.read()
                with sync_playwright() as p:
                    browser = p.chromium.launch(headless=True)
                    try:
                        context = browser.new_context(viewport={"width":1440,"height":1000})
                        page = context.new_page()
                        errors = []
                        page.on("pageerror", lambda e: errors.append(str(e)))
                        opened = []
                        def stub_open(route):
                            opened.append(route.request.post_data_json)
                            route.fulfill(status=200, content_type="application/json", body="null")
                        page.route("**/api/open_catalogue_page", stub_open)
                        page.goto(match.group(1).replace('/#', '/?route=library&splash=0#'))
                        page.get_by_role("button", name="Catalogue metadata · 0 entries").click()
                        page.get_by_label("Catalogue snapshot JSON").set_input_files(root / "export/catalog.json")
                        expect(page.get_by_role("button", name="Catalogue metadata · 43 entries")).to_be_visible()
                        search = page.locator('.search input')
                        search.fill("Addams Family Values")
                        page.get_by_label("Region", exact=True).select_option("Europe")
                        page.get_by_label("Language", exact=True).select_option("fr")
                        page.get_by_label("ROM revision", exact=True).select_option("1.0")
                        page.get_by_label("Player count", exact=True).select_option("1")
                        expect(page.locator(".grid .card")).to_have_count(1)
                        page.get_by_role("button", name="Addams Family Values", exact=True).click()
                        expect(page.get_by_role("heading", name="Source metadata · Vimm’s Lair")).to_be_visible()
                        expect(page.locator(".metadata")).to_contain_text("2")
                        expect(page.locator(".metadata")).to_contain_text("36950")
                        expect(page.locator(".metadata")).to_contain_text("2026-10-18")
                        expect(page.locator(".metadata")).to_contain_text("libretro-thumbnails")
                        expect(page.get_by_role("button", name="Play", exact=True)).to_have_count(0)
                        cover = page.locator(".banner-cover")
                        expect(cover).to_be_visible()
                        page.wait_for_function("document.querySelector('.banner-cover')?.naturalWidth > 0")
                        page.get_by_role("button", name="Open source page in browser", exact=True).click()
                        page.wait_for_timeout(100)
                        assert opened == [{"record_id":"vimm:43937"}]
                        page.reload()
                        # UI focus/route state need not survive reload; backend metadata must.
                        page.get_by_role("button", name="Library", exact=True).click()
                        expect(page.get_by_role("button", name="Catalogue metadata · 43 entries")).to_be_visible()
                        page.locator('.search input').fill("Addams Family Values")
                        page.get_by_role("button", name="Addams Family Values", exact=True).click()
                        expect(page.get_by_role("img", name="Addams Family Values box cover", exact=True)).to_be_visible()
                        page.wait_for_function("document.querySelector('.source-cover')?.naturalWidth > 0 && document.querySelector('.banner-cover')?.naturalWidth > 0")
                        assert page.locator('.page').evaluate("el => getComputedStyle(el).isolation") == 'isolate'
                        output = ROOT / "scripts/vimm/.poc"
                        output.mkdir(exist_ok=True)
                        page.screenshot(path=str(output / "marquee-details.png"), full_page=True)
                        page.get_by_role("button", name="Library", exact=True).click()
                        page.get_by_role("button", name="Catalogue metadata · 43 entries").click()
                        page.get_by_role("button", name="Remove imported catalogue & metadata links").click()
                        expect(page.get_by_role("button", name="Catalogue metadata · 0 entries")).to_be_visible()
                        assert not errors, errors
                        print("PASS: real Marquee UI → JSON import → region/language/revision/player filters → details/cover/provenance → browser handoff → reload persistence → removal")
                    finally:
                        browser.close()
            finally:
                process.terminate()
                try: process.wait(timeout=5)
                except subprocess.TimeoutExpired: process.kill(); process.wait()


if __name__ == "__main__": main()
