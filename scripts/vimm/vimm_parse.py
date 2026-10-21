"""Parse Vimm's Lair Vault HTML (live pages or saved samples)."""

from __future__ import annotations

import base64
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable
from urllib.parse import urljoin, urlparse

from bs4 import BeautifulSoup

VAULT_GAME_RE = re.compile(r"(?:^|/)vault/(\d+)(?:/|$|\?)")
ALL_MEDIA_RE = re.compile(r"let allMedia\s*=\s*(\[.*?\]);", re.DOTALL)
BLOCKED_RE = re.compile(r"var blocked_(\d+)\s*=\s*true")


@dataclass(frozen=True)
class VaultSystem:
    category: str
    name: str
    slug: str
    launch_year: str
    url: str


@dataclass(frozen=True)
class VaultListEntry:
    system: str
    section: str
    game_id: str
    title: str
    regions: str
    version: str
    languages: str
    rating: str
    url: str


@dataclass(frozen=True)
class VaultSection:
    label: str
    path: str


def _soup(html: str) -> BeautifulSoup:
    return BeautifulSoup(html, "html.parser")


def parse_systems(html: str, base_url: str = "https://vimm.net") -> list[VaultSystem]:
    soup = _soup(html)
    systems: list[VaultSystem] = []
    for table in soup.select("table.rounded caption"):
        caption = (table.get_text(strip=True) or "").strip()
        if caption not in ("Consoles", "Handhelds"):
            continue
        parent = table.find_parent("table")
        if not parent:
            continue
        for row in parent.select("tr"):
            link = row.find("a", href=True)
            if not link:
                continue
            href = link["href"]
            if "/vault/" not in href or href.rstrip("/").endswith("/vault"):
                continue
            slug = href.rstrip("/").split("/")[-1]
            year_span = row.select_one(".yearColumn span")
            year = year_span.get_text(strip=True) if year_span else ""
            systems.append(
                VaultSystem(
                    category=caption,
                    name=link.get_text(strip=True),
                    slug=slug,
                    launch_year=year,
                    url=urljoin(base_url, href),
                )
            )
    return systems


def parse_vault_sections(html: str) -> list[VaultSection]:
    menu = _soup(html).select_one("#vaultMenu")
    if not menu:
        return []
    sections: list[VaultSection] = []
    for link in menu.find_all("a", href=True):
        href = link["href"]
        label = link.get_text(strip=True) or ""
        if "section=number" in href:
            sections.append(VaultSection(label="#", path=href))
        else:
            path = urlparse(href).path
            sections.append(VaultSection(label=label, path=path))
    return sections


def _section_from_url(url: str) -> str:
    if "section=number" in url:
        return "#"
    path = urlparse(url).path.rstrip("/")
    tail = path.split("/")[-1]
    return tail if tail and tail != "Genesis" else ""


def parse_list_page(
    html: str,
    *,
    system: str,
    section: str,
    page_url: str,
    base_url: str = "https://vimm.net",
) -> list[VaultListEntry]:
    soup = _soup(html)
    table = soup.select_one("table.hovertable")
    if not table:
        return []
    entries: list[VaultListEntry] = []
    for row in table.select("tbody tr"):
        cells = row.find_all("td")
        if len(cells) < 5:
            continue
        title_link = None
        for a in cells[0].find_all("a", href=True):
            if VAULT_GAME_RE.search(a["href"]):
                title_link = a
                break
        if not title_link:
            continue
        m = VAULT_GAME_RE.search(title_link["href"])
        if not m:
            continue
        game_id = m.group(1)
        regions = " | ".join(
            img.get("title", "") for img in cells[1].select("img.flag") if img.get("title")
        )
        entries.append(
            VaultListEntry(
                system=system,
                section=section or _section_from_url(page_url),
                game_id=game_id,
                title=title_link.get_text(strip=True),
                regions=regions,
                version=cells[2].get_text(strip=True),
                languages=cells[3].get_text(strip=True),
                rating=cells[4].get_text(strip=True),
                url=urljoin(base_url, title_link["href"]),
            )
        )
    return entries


def parse_all_media(html: str) -> list[dict]:
    m = ALL_MEDIA_RE.search(html)
    if not m:
        return []
    return json.loads(m.group(1))


def decode_good_title(encoded: str | None) -> str:
    if not encoded:
        return ""
    try:
        return base64.b64decode(encoded).decode("utf-8", errors="replace")
    except Exception:
        return encoded


def blocked_var_name(html: str) -> str | None:
    m = BLOCKED_RE.search(html)
    return f"blocked_{m.group(1)}" if m else None


def read_html_file(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def load_sample_catalog(sample_dir: Path, system: str = "Genesis") -> list[VaultListEntry]:
    """Parse list samples under Marquee/sample/ for offline checks."""
    entries: list[VaultListEntry] = []
    for path in sorted(sample_dir.glob("*.html")):
        if "Genesis -" not in path.name and "Genesis)" not in path.name:
            continue
        html = read_html_file(path)
        if "hovertable" not in html:
            continue
        section = "A" if " - A" in path.name else ""
        entries.extend(
            parse_list_page(
                html,
                system=system,
                section=section,
                page_url=f"https://vimm.net/vault/{system}/{section or 'A'}",
            )
        )
    return entries


def systems_to_csv_rows(systems: Iterable[VaultSystem]) -> list[dict[str, str]]:
    return [
        {
            "category": s.category,
            "name": s.name,
            "slug": s.slug,
            "launch_year": s.launch_year,
            "url": s.url,
        }
        for s in systems
    ]


def catalog_to_csv_rows(entries: Iterable[VaultListEntry]) -> list[dict[str, str]]:
    return [
        {
            "system": e.system,
            "section": e.section,
            "game_id": e.game_id,
            "title": e.title,
            "regions": e.regions,
            "version": e.version,
            "languages": e.languages,
            "rating": e.rating,
            "url": e.url,
        }
        for e in entries
    ]
