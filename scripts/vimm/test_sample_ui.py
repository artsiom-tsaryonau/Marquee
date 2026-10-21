"""Explicit browser E2E test against a disposable local POC, never personal Chrome."""

from pathlib import Path
import threading

from playwright.sync_api import sync_playwright

from sample_flow import ROOT, create_server, load_snapshot


def main():
    snapshot = load_snapshot(ROOT / "sample")
    with create_server(snapshot) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with sync_playwright() as p:
                browser = p.chromium.launch(headless=True)
                try:
                    context = browser.new_context(viewport={"width": 1440, "height": 1000})
                    # Browser handoff is exercised with a stubbed response, no remote requests.
                    context.route("https://vimm.net/**", lambda route: route.fulfill(body="Live page handoff stub"))
                    page = context.new_page()
                    errors = []
                    page.on("pageerror", lambda error: errors.append(str(error)))
                    page.goto(f"http://127.0.0.1:{server.server_port}/")
                    assert page.locator("#systems button").count() == 36
                    assert page.locator("#games button").count() == 43
                    assert page.locator("#variant option").count() == 1
                    assert "36950" in page.locator("#details").inner_text()
                    page.locator("#search").fill("Addams Family Values")
                    assert page.locator("#games button").count() == 1
                    page.locator("#language").select_option("fr")
                    page.locator("#region").select_option("Europe")
                    assert page.locator("#games button").count() == 1
                    with page.expect_popup() as popup_info:
                        page.get_by_role("link", name="Open live page in a new tab").click()
                    popup = popup_info.value
                    popup.wait_for_load_state()
                    assert popup.url == "https://vimm.net/vault/43937"
                    assert popup.evaluate("window.opener === null")
                    popup.close()
                    page.locator("#search").fill("no matching game")
                    assert page.locator("#games button").count() == 0
                    page.locator("#search").fill("")
                    page.locator("#region").select_option("")
                    page.locator("#language").select_option("")
                    page.locator("#games button").first.click()
                    assert "No detail HTML saved" in page.locator("#details").inner_text()
                    page.get_by_role("button", name="PlayStation (0)", exact=True).click()
                    assert "No catalogue pages were saved" in page.locator("#games").inner_text()
                    page.get_by_role("button", name="Genesis (43)", exact=True).click()
                    page.locator("#search").fill("Addams Family Values")
                    page.locator("#games button").click()
                    assert not errors, errors
                    output = Path(__file__).parent / ".poc"
                    output.mkdir(exist_ok=True)
                    page.screenshot(path=str(output / "preview.png"), full_page=True)
                    page.set_viewport_size({"width": 390, "height": 844})
                    assert page.evaluate("document.documentElement.scrollWidth <= innerWidth")
                    print("PASS: systems → filters → detail/media → new-tab handoff; empty states; no page errors; mobile width")
                finally:
                    browser.close()
        finally:
            server.shutdown()
            thread.join()


if __name__ == "__main__":
    main()
