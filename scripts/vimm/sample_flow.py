#!/usr/bin/env python3
"""Build or serve an offline catalogue from the three saved Vault HTML pages."""

from __future__ import annotations

import argparse
import base64
from dataclasses import asdict
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

from bs4 import BeautifulSoup

from vimm_browser import open_native_tab
from vimm_parse import VAULT_GAME_RE, decode_good_title, parse_all_media, parse_list_page, parse_systems

ROOT = Path(__file__).resolve().parents[2]
FILES = {
    "systems": "Vimm's Lair_ The Vault.html",
    "catalog": "The Vault - Genesis - A.html",
    "detail": "The Vault_ Addams Family Values (Genesis).html",
}


def detail_metadata(html: str, sample_dir: Path) -> dict:
    soup = BeautifulSoup(html, "html.parser")
    values = {}
    for row in soup.select("table.rounded tr"):
        cells = row.find_all("td", recursive=False)
        if len(cells) == 3:
            values[cells[0].get_text(" ", strip=True)] = cells[2].get_text(" ", strip=True)
    def number(label):
        value = values.get(label, "").split()
        try:
            return float(value[0]) if value else None
        except ValueError:
            return None
    votes = re.search(r"\((\d+) votes?\)", values.get("Overall", ""))
    verified = soup.select_one("#data-date")
    cover = None
    image = soup.select_one('img[alt="Box"]')
    if image and image.get("src"):
        path = (sample_dir / unquote(str(image["src"]))).resolve()
        if path.is_relative_to(sample_dir.resolve()) and path.is_file() and path.stat().st_size <= 2_000_000:
            mime = {".avif": "image/avif", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg"}.get(path.suffix.lower())
            if mime:
                cover = {"data_url": f"data:{mime};base64,{base64.b64encode(path.read_bytes()).decode()}",
                         "credit": "Box cover displayed by Vimm's Lair; page credits libretro-thumbnails.",
                         "source_url": "https://github.com/libretro-thumbnails/"}
    year = number("Year")
    players = number("Players")
    return {"players": int(players) if players and players.is_integer() else None,
            "year": int(year) if year and year.is_integer() else None,
            "ratings": {key: number(label) for key, label in [("graphics", "Graphics"), ("sound", "Sound"), ("gameplay", "Gameplay"), ("overall", "Overall")]},
            "rating_votes": int(votes.group(1)) if votes else None,
            "verified_at": verified.get_text(strip=True) if verified else None,
            "cover": cover}


def provider_url(url: str) -> str:
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc != "vimm.net" or not parsed.path.startswith("/vault"):
        raise ValueError(f"Unexpected provider URL: {url!r}")
    return url


def page_url(html: str) -> str:
    meta = BeautifulSoup(html, "html.parser").select_one('meta[property="og:url"]')
    if not meta or not meta.get("content"):
        raise ValueError("Saved page is missing its og:url identity")
    return provider_url(str(meta["content"]))


def load_snapshot(sample_dir: Path) -> dict:
    """Extract only whitelisted metadata. Never include source HTML or tokens."""
    pages = {}
    for role, name in FILES.items():
        path = sample_dir / name
        if not path.is_file():
            raise ValueError(f"Missing {role} sample: {path}")
        pages[role] = path.read_text(encoding="utf-8")

    systems = [asdict(s) for s in parse_systems(pages["systems"])]
    if not systems:
        raise ValueError("Systems sample parsed no systems; check the saved page/selectors")
    catalog_url = page_url(pages["catalog"])
    parts = urlsplit(catalog_url).path.strip("/").split("/")
    if len(parts) != 3:
        raise ValueError("Expected a section catalogue URL: /vault/SYSTEM/SECTION")
    system, section = parts[1:]
    if system not in {s["slug"] for s in systems}:
        raise ValueError(f"Catalogue system {system!r} is absent from the systems sample")
    entries = parse_list_page(pages["catalog"], system=system, section=section, page_url=catalog_url)
    if not entries:
        raise ValueError("Catalogue sample parsed no games; check the saved page/selectors")
    games = {}
    for entry in entries:
        game = asdict(entry)
        game["url"] = provider_url(game["url"])
        game["media"] = []
        game["has_detail"] = False
        game.update({"players": None, "year": None, "ratings": {}, "rating_votes": None, "verified_at": None, "cover": None})
        games[game["game_id"]] = game

    detail_url = page_url(pages["detail"])
    match = VAULT_GAME_RE.search(urlsplit(detail_url).path)
    if not match or match.group(1) not in games:
        raise ValueError("Detail sample does not match any game in the catalogue sample")
    selected = games[match.group(1)]
    try:
        media = parse_all_media(pages["detail"])
    except (ValueError, TypeError) as exc:
        raise ValueError("Detail sample contains malformed media JSON") from exc
    if not isinstance(media, list) or not media:
        raise ValueError("Detail sample parsed no media variants")
    for item in media:
        if not isinstance(item, dict) or not isinstance(item.get("ID"), int):
            raise ValueError("Detail sample has a media variant without a numeric ID")
        filename = decode_good_title(item.get("GoodTitle"))
        groups = re.findall(r"\(([^)]+)\)", filename)
        known_regions = {"Europe", "USA", "Japan", "World", "Australia", "Brazil", "Korea", "China", "Taiwan", "Asia", "Canada", "France", "Germany", "Italy", "Spain", "UK", "Russia"}
        regions = [part.strip() for group in groups for part in group.split(",") if part.strip() in known_regions]
        languages = next(([code.lower() for code in group.split(",")] for group in groups if re.fullmatch(r"[A-Z][a-z](?:,[A-Z][a-z])*", group)), [])
        good_date = item.get("GoodDate")
        selected["media"].append({
            "media_id": str(item["ID"]),
            "filename": filename,
            "regions": regions,
            "languages": languages,
            "verified_at": str(good_date.get("date", ""))[:10] if isinstance(good_date, dict) else None,
            "version": str(item.get("Version") or ""),
            "size": str(item.get("ZippedText") or ""),
            "serial": str(item.get("Serial") or ""),
            "crc32": str(item.get("GoodHash") or ""),
            "md5": str(item.get("GoodMd5") or ""),
            "sha1": str(item.get("GoodSha1") or ""),
        })
    selected["has_detail"] = True
    selected.update(detail_metadata(pages["detail"], sample_dir))
    for row in systems:
        row["url"] = provider_url(row["url"])
        row["sample_games"] = len(games) if row["slug"] == system else 0
    return {
        "schema_version": 1,
        "provider": "vimm",
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "source": "Saved HTML snapshots — not a live availability check",
        "system": system,
        "section": section,
        "systems": systems,
        "games": list(games.values()),
        "detail_game_id": selected["game_id"],
        "input_files": list(FILES.values()),
    }


def render_html(snapshot: dict) -> bytes:
    template = Path(__file__).with_name("sample_ui.html").read_text(encoding="utf-8")
    # A title containing </script> must remain JSON data, never become markup.
    payload = json.dumps(snapshot, ensure_ascii=True).replace("<", "\\u003c")
    return template.replace("__SNAPSHOT_JSON__", payload).encode("utf-8")


def build(snapshot: dict, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    (output / "catalog.json").write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")
    (output / "index.html").write_bytes(render_html(snapshot))


def create_server(snapshot: dict, port: int = 0) -> ThreadingHTTPServer:
    html = render_html(snapshot)
    catalog = json.dumps(snapshot).encode("utf-8")

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.headers.get("Host") not in {
                f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"
            }:
                self.send_error(403)
                return
            path = urlsplit(self.path).path
            if path in {"/", "/index.html"}:
                body, content_type = html, "text/html; charset=utf-8"
            elif path == "/catalog.json":
                body, content_type = catalog, "application/json; charset=utf-8"
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("X-Frame-Options", "DENY")
            self.send_header("Content-Security-Policy", "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")
            self.end_headers()
            self.wfile.write(body)

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "serve"])
    parser.add_argument("--sample-dir", type=Path, default=ROOT / "sample")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).parent / ".poc")
    parser.add_argument("--port", type=int, default=0, help="Loopback port; 0 picks a free port")
    parser.add_argument("--open", action="store_true", help="Open the POC in normal Chrome")
    parser.add_argument("--browser", help="Chrome/Chromium executable")
    parser.add_argument("--profile", help="Chrome internal profile directory name")
    args = parser.parse_args()
    if args.command == "build" and args.open:
        parser.error("Use serve --open to open the POC in Chrome")
    try:
        snapshot = load_snapshot(args.sample_dir)
        print(f"Parsed {len(snapshot['systems'])} systems, {len(snapshot['games'])} games, 1 detail page.", flush=True)
        if args.command == "build":
            build(snapshot, args.output_dir)
            print(f"Built {args.output_dir.resolve() / 'index.html'}")
            return 0
        with create_server(snapshot, args.port) as server:
            url = f"http://127.0.0.1:{server.server_port}/"
            print(f"POC: {url}\nPress Ctrl+C to stop.", flush=True)
            if args.open:
                open_native_tab(url, browser=args.browser, profile=args.profile)
            server.serve_forever()
    except KeyboardInterrupt:
        return 0
    except (ValueError, OSError, RuntimeError) as exc:
        parser.exit(1, f"POC failed: {exc}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
